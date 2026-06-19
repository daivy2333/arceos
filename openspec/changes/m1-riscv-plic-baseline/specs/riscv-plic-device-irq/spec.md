# riscv-plic-device-irq Delta Specification

> Version: 0.1.0
> Last updated: 2026-06-19

## ADDED Requirements

### Requirement: PLIC context 初始化

RISC-V QEMU virt platform SHALL 为每个完成 platform later-init 的 hart 初始化对应 supervisor PLIC context，并允许该 hart 接收 supervisor external interrupt。

#### Scenario: BSP 初始化
- **WHEN** BSP 执行 platform later-init
- **THEN** 系统初始化 context `hart_id * 2 + 1`、threshold 和 supervisor external interrupt enable

#### Scenario: AP 初始化
- **WHEN** SMP 配置下任一 AP 执行 platform later-init
- **THEN** 系统只初始化该 AP 对应的 supervisor context，不覆盖其他 hart 的 enable 状态

### Requirement: 设备 IRQ enable 与 disable

PLIC 实现 SHALL 将 `axhal::irq::set_enable(device_irq, enabled)` 映射为当前 supervisor context 的 source priority 和 enable bit 操作。

#### Scenario: Enable UART IRQ 10
- **WHEN** 调用 `set_enable(10, true)`
- **THEN** IRQ 10 获得非零 priority，并在当前 supervisor context 中启用

#### Scenario: Disable UART IRQ 10
- **WHEN** 已启用 IRQ 10 后调用 `set_enable(10, false)`
- **THEN** 当前 supervisor context 不再接收 IRQ 10，其他 IRQ source 状态保持不变

#### Scenario: IRQ 0
- **WHEN** 调用 `set_enable(0, enabled)`
- **THEN** 系统安全返回且不访问无效 PLIC source 0

### Requirement: External IRQ claim、分派与 complete

收到 supervisor external interrupt 时，platform SHALL 从当前 supervisor context claim 实际设备 IRQ，调用该 IRQ 的 handler，并在 handler 返回后 complete 同一 IRQ。

#### Scenario: UART IRQ 正常分派
- **WHEN** UART0 触发 PLIC IRQ 10 且 IRQ 10 已注册并启用
- **THEN** IRQ 10 handler 恰好处理该次 claim，随后系统 complete IRQ 10

#### Scenario: 未注册设备 IRQ
- **WHEN** PLIC claim 到未注册的设备 IRQ
- **THEN** 系统记录 unhandled 状态并仍 complete 该 IRQ，不形成 IRQ storm

#### Scenario: Spurious external interrupt
- **WHEN** supervisor external trap 到达但 PLIC claim 返回无 IRQ
- **THEN** 系统不调用设备 handler、不执行无效 complete，并安全返回

### Requirement: 注册生命周期

设备 IRQ 注册 SHALL 继续使用现有 `axhal::irq::{register, unregister}` API，并保持 handler table 的唯一注册语义。

#### Scenario: 首次注册
- **WHEN** IRQ 10 尚未注册且调用 `register(10, handler)`
- **THEN** 注册成功并启用 IRQ 10

#### Scenario: 重复注册
- **WHEN** IRQ 10 已有 handler 且再次注册
- **THEN** 第二次注册失败，原 handler 保持不变

#### Scenario: 注销
- **WHEN** 已注册 IRQ 10 后调用 `unregister(10)`
- **THEN** 系统返回原 handler 并禁用 IRQ 10

### Requirement: 跨架构隔离与 console 保持

PLIC 变更 MUST 仅影响 `riscv64-qemu-virt`，且 MUST 不改变现有 SBI console 行为。

#### Scenario: RISC-V 启动日志
- **WHEN** 使用 irq feature 启动 RISC-V QEMU virt
- **THEN** SBI console 在 PLIC 初始化前后均可输出启动与故障日志

#### Scenario: 非 RISC-V 构建
- **WHEN** 构建任一现有非 RISC-V 默认平台
- **THEN** 该平台不编译 vendor PLIC 实现且既有 IRQ API 行为不变

