## Context

当前 ArceOS 的 `modules/axhal/src/irq.rs` 只是 `axplat::irq` 门面。实际 RISC-V platform 来自 crates.io `axplat-riscv64-qemu-virt 0.4.1`，其中 `set_enable()` 仅 warning，external trap 固定调用 `HandlerTable::handle(0)`，不存在 claim/complete。StarryOS 使用的 platform `0.3.1-pre.6` 已通过 `riscv_plic 0.2.0` 完成同一硬件路径并验证 UART IRQ 10。

影响分析表明 ArceOS 内部 `irq_handler` 只有 `axhal` 本文件直接影响；真正 blast radius 位于 platform crate、所有 RISC-V 设备 IRQ 以及 SMP context。

## Goals / Non-Goals

**Goals:**

- 使 RISC-V QEMU virt 可正确注册和处理设备 IRQ。
- 用 UART IRQ 10 建立可重复的 RED/GREEN QEMU 见证。
- 保持现有 `axhal::irq` API 和 SBI console。
- 为 M3 async UART 提供可信硬件中断前置条件。

**Non-Goals:**

- 不集成 `uart_16550/async`、copier、ring buffer 或 Future。
- 不修改 `axhal` 通用 IRQ handler 签名。
- 不设计通用中断控制器抽象或支持 nested IRQ。
- 不以本 change 验收多 UART、TTY、stdin/stdout 或性能。

## Decisions

### 1. 使用仓库内 vendor patch

在 `vendor/axplat-riscv64-qemu-virt` 保存 0.4.1 源码并通过根 `[patch.crates-io]` 替换。

原因：当前仓库无 sibling `axplat_crates` 可写约束；vendor patch 可在同一分支审查、测试和回滚，同时不污染 `modules/axhal` 的平台边界。

备选：整体降级到 0.3.1-pre.6 会引入 `axplat` trait 签名差异；直接修改 Cargo registry 不可追踪；把 PLIC 塞入 `axhal` 违反 platform 分层。

### 2. 移植 0.3.1-pre.6 的 PLIC 状态机

复用以下结构，不改变 0.4.1 `IrqIf::handle -> ()` 接口：

```text
S_EXT trap
  -> lock PLIC
  -> claim(this_context)
  -> unlock PLIC
  -> HandlerTable::handle(claimed_irq)
  -> lock PLIC
  -> complete(this_context, claimed_irq)
  -> unlock PLIC
```

handler 执行期间不持有 PLIC lock，避免 handler 间接操作 IRQ 时自死锁。与参考实现相比，这是必须明确保留的锁边界。

### 3. supervisor context 按 hart 计算

使用 `this_cpu_id() * 2 + 1`。`init_percpu()` 初始化当前 context，并打开 `sie.ssoft/stimer/sext`。source enable/disable 使用调用 CPU 的 context。

备选：固定 context 1 只适用于单核，会使 SMP 配置在 AP 上错误 claim。

### 4. M1 测试见证使用同步 UART probe

新增 `examples/uart_irq`：用 `uart_16550` 同步 API 配置 UART RX interrupt，注册 IRQ 10 handler，只做中断原因确认和原子计数。测试不启用 async feature。

RED：未应用 PLIC 变更时输入字符后 handler 计数不增加，日志出现当前 platform 的 `set_enable is not implemented` 或 unhandled IRQ 0。

GREEN：输入字符后 IRQ 10 计数增加；disable 期间不增加；重新 enable 后恢复；退出时无重复中断。

备选：纯 MMIO fake 无法证明 QEMU PLIC wiring；直接使用 async driver 会把 M2/M3 引入 M1。

## 中断序列

```text
QEMU UART0       PLIC                 axhal/axplat               probe
    | IRQ 10       |                        |                      |
    |------------->|                        |                      |
    |               |---- S_EXT trap ------>|                      |
    |               |<--- claim(ctx) -------|                      |
    |               |---- irq=10 ---------->|                      |
    |               |                        |---- handler() ------>|
    |               |                        |<--- return ----------|
    |               |<--- complete(10) ------|                      |
```

## Requirements Traceability Matrix

| Requirement | Task(s) | Coverage | Simplification | Status |
|-------------|---------|----------|----------------|--------|
| PLIC per-hart context 初始化 | 2.1, 2.2, 4.2 | unit/compile + QEMU | None | ✅ |
| IRQ enable/disable | 2.3, 3.2, 4.2 | IRQ probe | None | ✅ |
| claim/dispatch/complete | 2.4, 3.1, 4.2 | IRQ probe + counters | None | ✅ |
| spurious/unregistered IRQ | 2.4, 3.3, 4.3 | targeted test/log assertion | None | ✅ |
| register/unregister 生命周期 | 3.2, 3.3 | duplicate/disable scenarios | None | ✅ |
| 跨架构与 SBI console 不回归 | 4.1, 4.4 | build matrix + boot output | None | ✅ |

## Risks / Trade-offs

- [Vendor fork 漂移] → 固定原始 0.4.1 版本和补丁说明，后续 upstream 修复时用 diff 迁移。
- [PLIC lock 跨 handler 死锁] → claim 后释放锁，handler 返回后重新加锁 complete。
- [早到 IRQ] → probe 顺序固定为 UART state/handler 完成后才 enable source 与 UART IER。
- [SMP context 错误] → 单独编译 SMP，并在每 hart later-init 记录 context；M1 运行 Gate 以单核为主，SMP 只验证初始化隔离。
- [QEMU 交互测试不稳定] → probe 输出机器可解析计数，并使用带超时的输入脚本。

## Migration Plan

1. 建立 vendor patch，但尚不实现 PLIC，确认当前 RED 见证。
2. 添加 `riscv_plic 0.2.0` 和 platform 配置常量。
3. 实现 context 与 enable/disable。
4. 实现 claim/dispatch/complete。
5. 运行 UART IRQ probe、RISC-V boot 和非 RISC-V build Gate。
6. 保留 vendor patch供 M3 使用。

回滚：移除根 `[patch.crates-io]` 后重新锁定 Cargo.lock；删除 vendor 和 probe。该回滚不修改 ArceOS 公共 API 或数据格式。

## Open Questions

无。vendor 载体、依赖版本、context 算法、测试设备和范围均已在本计划中固定。

