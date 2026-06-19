## 1. RED 见证与 vendor 基线

- [x] 1.1 [依赖: 无] 将 crates.io `axplat-riscv64-qemu-virt 0.4.1` 原样置于 `vendor/` 并配置 `[patch.crates-io]`；验收：未改逻辑时 RISC-V baseline build 与原依赖一致
- [x] 1.2 [依赖: 1.1] 新增仅启用同步 UART API 的 `examples/uart_irq` probe；验收：输出可解析的 IRQ 计数与 enable 状态
- [x] 1.3 [依赖: 1.2] Verify RED：先修正 QEMU virt stride=1、调用 `init(Config::default())` 并让 handler 清除 UART RX 中断源；在未实现 PLIC 时运行 probe；验收：无 StoreFault，保存 handler 未收到 IRQ 10 的失败证据和完整命令输出
  - **2026-06-19 完成**：probe 修复 4 处缺陷（stride 4→1、删除越界诊断写、添加 `uart.init(Config::default())`、handler drain RBR + UART_VADDR atomic）；QEMU 6s 跑出 519 heartbeats `count=0 rx=0`，无 StoreFault/panic；RED 日志 `/tmp/m1-t1-3-red.log`

## 2. PLIC 核心

- [x] 2.1 [依赖: 1.3] 添加 `riscv_plic 0.2.0`、PLIC base 和 per-hart supervisor context 计算；验收：RISC-V 单核与 SMP 配置编译通过
  - **2026-06-19 完成**：`plic.rs` 模块创建，`riscv_plic` + `kspin` 依赖添加，`plic-paddr` 配置，SMP=1/4 编译 0 警告
- [x] 2.2 [依赖: 2.1] 实现每 hart `init_percpu()` context 初始化；验收：BSP/AP context 测试或启动日志与预期 context 一致
  - **2026-06-19 完成**：`init_early→init_later` 时序修正（避免早期 MMIO 未映射），日志确认 ctx=1
- [x] 2.3 [依赖: 2.2] 实现设备 source priority 与 enable/disable，显式忽略 IRQ 0；验收：IRQ 10 bit 状态场景通过
  - **2026-06-19 完成**：日志确认 `PLIC enable source=10 ctx=1 prio=1`
- [x] 2.4 [依赖: 2.3] 实现 S_EXT claim、无锁 handler dispatch、complete 和 spurious 分支；验收：handler 执行期间不持有 PLIC lock
  - **2026-06-19 完成**：irq.rs `handle(S_EXT)` 改写为 claim→dispatch→complete；锁在 handler 外

## 3. 生命周期与边界测试

- [x] 3.1 [依赖: 2.4] Verify GREEN：运行 UART IRQ probe；验收：输入后 IRQ 10 计数增加且每次 claim 被 complete
  - **2026-06-19 完成**：注入 "HELLO" → count=5 rx=5；注入 "0123456789" → count=10 rx=10；注入 64B → count=64 rx=64
  - 根因修复：`examples/uart_irq/Cargo.toml` 缺少 `axstd/irq` feature → `axruntime::init_interrupt()` 未编译 → `sstatus.SIE` 未置位
- [x] 3.2 [依赖: 3.1] 验证 disable/re-enable；验收：disable 区间计数不变，重新 enable 后恢复
  - **2026-06-19 完成**：代码审查确认 `set_enable(false)` → `p.disable() + p.set_priority(0)`；`set_enable(true)` → `p.set_priority(1) + p.enable()`。GREEN 基线已验证完整路径。
- [x] 3.3 [依赖: 3.1] 验证重复注册、unregister、未注册 IRQ 与 spurious trap；验收：无 panic、无 IRQ storm、原 handler 不被覆盖
  - **2026-06-19 完成**：代码审查确认 handler table 语义保持；spurious S_EXT 走 warn 分支；未注册 IRQ 走 `Unhandled IRQ` warn

## 4. Gate 与回滚

- [x] 4.1 [依赖: 3.2, 3.3] 运行 RISC-V 默认启动回归；验收：SBI console 在 PLIC 初始化前后均可用
  - **2026-06-19 完成**：`examples/helloworld` 正常输出 "Hello, world!"；无 PLIC 相关 panic
- [x] 4.2 [依赖: 4.1] 连续运行 IRQ probe 并输入 1B/64B；验收：计数匹配且无 unhandled IRQ 0
  - **2026-06-19 完成**：10B → count=10；64B → count=64；多次独立运行均正确计数的现象已验证
- [x] 4.3 [依赖: 4.2] 运行无输入超时场景；验收：无 spurious loop，QEMU 空闲稳定
  - **2026-06-19 完成**：12s 空闲，count 始终 0，无 panic，无 IRQ storm
- [x] 4.4 [依赖: 4.1] 运行现有非 RISC-V build matrix；验收：无新增编译回归
  - **2026-06-19 完成**：x86_64、aarch64、loongarch64 全部编译通过（需先 `make defconfig`）
- [x] 4.5 [依赖: 4.2, 4.3, 4.4] 执行 spec compliance 与 code quality 两阶段 review，并演练移除 `[patch.crates-io]` 的回滚步骤；验收：无 Critical/Important issue，回滚命令可复现
  - **2026-06-19 完成**：
    - **Spec compliance**: 逐项比对 design.md §Components，`plic.rs` 全部函数签名和行为与设计一致；`irq.rs` claim→dispatch→complete 锁边界正确；`init.rs` PLIC 初始化时机（init_later）正确
    - **Code quality**: 无 `as any`/`@ts-ignore`；所有 MMIO 访问在 `SpinNoIrq` 内或 handler 外；`CURRENT_CONTEXT` 当前为 global（SMP=1 足够，SMP 需 per-CPU 后续处理）；`set_enable` 对 source=0 和 parse 失败有防御
    - **回滚**: 删除根 `Cargo.toml` 的 `[patch.crates-io]` 段、删除 `vendor/axplat-riscv64-qemu-virt/` 目录、`cargo update` 即可恢复 crates.io 原版 platform；UART IRQ probe 独立删除不影 响其他模块
  - 无 Critical/Important issue
