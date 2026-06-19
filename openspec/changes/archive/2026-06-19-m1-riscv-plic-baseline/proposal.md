## Why

当前 `axplat-riscv64-qemu-virt 0.4.1` 未实现 PLIC source enable、claim 和 complete，所有 supervisor external interrupt 都被错误分派为 IRQ 0。这直接阻塞 UART IRQ 10，也使任何 RISC-V 设备中断驱动 I/O 无法建立可信基线。

## What Changes

- 在仓库内引入可审计、可回滚的 `axplat-riscv64-qemu-virt 0.4.1` vendor patch，并通过 `[patch.crates-io]` 仅替换该 platform crate。
- 参考已在 StarryOS 使用的 `0.3.1-pre.6` 实现，引入 `riscv_plic 0.2.0`，实现 supervisor context 初始化、priority、enable/disable、claim/dispatch/complete。
- 增加只使用同步 `uart_16550` API 的 UART IRQ 10 probe，作为 PLIC 的 RED/GREEN QEMU 见证。
- 保持 `axhal::irq::{register, set_enable, unregister}` API、SBI console 和其他架构行为不变。
- 不接入 async UART、Future executor、TTY 或 stdio。

## Capabilities

### New Capabilities

- `riscv-plic-device-irq`: RISC-V QEMU virt 的设备 IRQ 注册、屏蔽、claim、分派和 complete 行为。

### Modified Capabilities

无。

## Impact

- 依赖与构建：根 `Cargo.toml`、`Cargo.lock`、新增 vendor platform crate、`riscv_plic 0.2.0`。
- 平台：仅 `riscv64-qemu-virt` 的 `irq` feature；x86_64/aarch64/loongarch64/armv7a 不改变。
- 中断安全：PLIC MMIO 访问处于 `SpinNoIrq` 临界区，handler 仍在 `NoPreempt` IRQ 上下文执行。
- SMP：每 hart 使用 supervisor context `hart_id * 2 + 1`，enable 与 claim/complete 绑定当前 context。
- 回滚：删除 `[patch.crates-io]` 和 vendor 目录即可恢复 crates.io 0.4.1；UART IRQ probe 独立删除。

