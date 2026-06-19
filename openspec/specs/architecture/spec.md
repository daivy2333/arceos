## Purpose

定义 arceos 模块化操作系统/单内核的架构决策和设计原则,指导内核子系统设计、跨架构抽象、硬件驱动开发和异步 I/O 演进的统一方向。

> Version: 0.1.0  
> Last updated: 2026-06-19  
> Scope: 全项目(workspace)

## Requirements

### Requirement: ADR 强制记录

所有影响系统架构、内核子系统边界、跨架构抽象或硬件驱动模型的决策 SHALL 以 ADR(Architecture Decision Record)条目记录在本文,包含决策标题、上下文、决策内容、影响范围、备选方案。

#### Scenario: 做出架构决策时

- **WHEN** 开发者完成一项影响 ≥1 个 modules/ 子系统或 ≥1 种架构的选型(如调度算法、内存分配策略、I/O 模型、驱动抽象)
- **THEN** 必须在本文新增一条 `### ADR-NNN: {标题}` 条目,附 ADR-NNN(状态/日期/上下文/决策/影响/备选)小节

#### Scenario: 查阅既有决策

- **WHEN** 开发者评估某项设计或重构时
- **THEN** 可通过 grep `ADR-` 或 `### ADR-` 关键词定位相关决策,确认是否与既定方向冲突

### Requirement: 模块化分层原则

arceos 内核 SHALL 保持 modules/(axhal/axdriver/axnet/axtask/axsync/...)分层模型,层次间通过最小接口解耦,禁止跨层反向依赖。

#### Scenario: 添加新模块

- **WHEN** 开发者引入新的 modules/<name> 内核模块
- **THEN** 必须声明其在分层中的位置(硬件抽象层/驱动层/核心服务层),Cargo.toml workspace.members 注册,并通过最小 trait 暴露能力

#### Scenario: 跨层调用审查

- **WHEN** 核心服务层模块(如 axtask)直接依赖具体硬件(如 axdriver::uart_ns16550a)
- **THEN** 必须重构为通过 axhal 或 trait 间接访问,否则视为架构违规

### Requirement: 跨架构一致接口

所有架构相关实现(x86_64/riscv64/aarch64/loongarch64/armv7a) SHALL 通过统一 trait 暴露能力,业务逻辑层不感知架构差异。

#### Scenario: 新增硬件特性

- **WHEN** 引入新硬件特性(如新的中断控制器、UART、SMP IPI)
- **THEN** 在 axhal 内为每种支持的架构提供实现,新架构通过实现相同 trait 接入,业务代码无需改动

#### Scenario: 架构特定代码越界

- **WHEN** 在 modules/axnet、modules/axtask 等非 axhal 模块中直接使用 cfg(target_arch) 或 asm!
- **THEN** 视为越界,需重构到 axhal 或独立 arch 子模块

### Requirement: 异步 I/O 与中断驱动 I/O 演进

中断驱动设备 I/O 与异步 I/O SHALL 沿模块化、可中断安全、可 SMP 安全三条主线演进,新驱动/新 I/O 路径不得引入全局锁或睡眠中断路径。

#### Scenario: 实现新驱动

- **WHEN** 在 modules/axdriver 中新增驱动(UART/NIC/Block/GPU)
- **THEN** 必须提供中断注册/注销接口,实现 top-half(轻量)和 bottom-half(可调度任务)分离,SMP 场景下使用 per-CPU 数据或细粒度锁

#### Scenario: 引入异步 I/O 抽象

- **WHEN** 引入 async/await I/O 路径(async-uart-1 分支主线)
- **THEN** 异步 runtime 复用 axruntime/axtask,不重新引入 Tokio 等大依赖;Future 不得跨越中断上下文

### Requirement: Unikernel 与多用户态兼容并存

arceos SHALL 同时支持 Unikernel 模式(单地址空间,无 Linux 兼容)和向 Linux 应用兼容演进的路径,核心模块不应假设单一模式。

#### Scenario: 评估用户态接口变更

- **WHEN** 修改 api/arceos_api、api/arceos_posix_api 或 ulib/axstd、ulib/axlibc
- **THEN** 需标注对 Unikernel 和 Linux 兼容两种模式的兼容性影响,破坏性变更需要 ADR 记录

## ADR Index

### ADR-001: 模块化分层 + Cargo workspace

- **Status**: Accepted (历史基线,模块化分层决定)
- **Date**: 2024-*
- **Context**: arceos 受 Unikraft 启发,目标是模块化 OS,可裁剪为 unikernel 或通用内核。需要子模块可独立 version、复用、可裁剪。
- **Decision**: 采用 Cargo workspace,16 个 modules/(子 crate) + api/(用户态接口) + ulib/(用户态库) + examples/(示例) 的目录组织;模块间最小依赖,通过 trait 抽象。
- **Impact**: 1) 模块独立版本化(tags v0.2.0 等);2) 跨项目复用(`axalloc = { git = "...", tag = "v0.2.0" }`);3) 构建时间增加,需 feature 精细控制。
- **Alternatives**: 单 crate + modules/ 目录(放弃独立版本化);按 Linux kernel 风格的 make + Kconfig(放弃 Rust 生态)。

### ADR-002: axhal 作为唯一架构抽象层

- **Status**: Accepted
- **Date**: 2024-*
- **Context**: 跨 5 种架构(x86_64/riscv64/aarch64/loongarch64/armv7a)需要统一抽象,但又要保留每种架构的特殊能力(SBI/PLIC/IOAPIC/GIC/CSRCPUI 等)。
- **Decision**: 所有架构特定代码收敛到 modules/axhal,通过 cfg(target_arch) 分发;axhal 之外不直接写汇编或架构寄存器。
- **Impact**: 1) axhal 是最大依赖中心;2) 新架构接入门槛高,需要熟悉 axhal 现有 trait 体系;3) 测试需要覆盖每种架构 QEMU。
- **Alternatives**: 每种架构独立 crate(放弃接口统一);per-arch Cargo feature(复杂度膨胀,放弃)。

### ADR-003: axdriver 与 axhal 解耦

- **Status**: Accepted
- **Date**: 2024-*
- **Context**: 驱动既要复用 axhal 的硬件能力,又要独立可裁剪(README 中通过 FEATURES 选择)。
- **Decision**: axdriver 内置多种外设驱动(ns16550a/i8259/virtio-net 等),每个驱动实现统一 trait,UART/NIC/Block 等按 feature 开关。
- **Impact**: 1) 驱动需要每种架构的 board 适配;2) 中断注册/调度统一在 axhal/irq;3) 与 axruntime 调度器协作。
- **Alternatives**: 把驱动直接嵌入 axhal(架构抽象层膨胀);独立 OS crate per driver(跨项目协作困难)。

### ADR-004: 中断驱动 I/O + 异步 I/O 是当前主线

- **Status**: Accepted (async-uart-1 分支承接)
- **Date**: 2026-06-15
- **Context**: README 中 TODO 列表最后两项「Interrupt driven device I/O」「Async I/O」仍未完成,async-uart-1 分支开始为同步串口实现异步化。
- **Decision**: 沿「驱动中断化 → 异步 I/O 抽象 → runtime 集成」三步走;第一阶段以 UART(异步串口)为切入点,因为 16550 协议简单、QEMU 模拟稳定。
- **Impact**: 1) axdriver 需补齐中断 top-half/bottom-half;2) axruntime/axtask 需提供 Future 唤醒接口;3) ulib/axstd 暴露 async read/write API。
- **Alternatives**: 直接引入 Tokio + smoltcp(生态丰富但内核态引入过重);仅做同步中断驱动不做 async(放弃应用层体验)。

<!-- A01 -->
### ADR-005: 异步 UART 采用 early console 与 late device 分阶段集成

- **Status**: Proposed
- **Date**: 2026-06-19
- **Context**: `riscv64-qemu-virt` 当前 console 走 SBI，PLIC 设备中断未完成；`axtask` 尚无 Future executor。直接替换全局 console 会把启动诊断、平台 IRQ、驱动状态机和异步调度绑定成一次高风险变更。
- **Decision**: 保留 SBI 作为 early/fallback console；先完成 PLIC，再将 `uart_16550` 通过窄 OS adapter 作为独立 late device 接入；依次验证同步 ring-backed stdin、readiness 和 async stdio，最后再决定普通 stdout 是否切换到 UART。
- **Impact**: 需要可修改的 RISC-V platform crate、最小 Future executor adapter、per-port IRQ trampoline 和明确的 SPSC 所有权；集成步骤增加，但每层可独立测试和回退。
- **Alternatives**: 直接让异步 UART 替换 `ConsoleIf`（early boot 无 scheduler/IRQ 且故障时丢失日志）；仅在 x86 上验证（可绕过 PLIC，但不能证明 RISC-V 主目标）；继续 SBI 轮询（无法完成中断驱动 UART 目标）。

<!-- A02 -->
### ADR-006: 以最小能力回移复现 StarryOS parity，而非整体升级 ArceOS

- **Status**: Proposed
- **Date**: 2026-06-19
- **Context**: StarryOS 已在 `axtask/axhal 0.3.0-preview.2` 与 RISC-V platform `0.3.1-pre.6` 上验证异步 UART；当前仓库是 ArceOS 0.2，缺少 Future `block_on`，所用 platform 0.4.1 又缺失 PLIC。整体升级会把大量无关 API 迁移引入当前主线。
- **Decision**: 只回移两个前置能力：参考 0.3.1-pre.6 修复 platform 0.4.1 PLIC，参考 axtask 0.3 增加单 Future `block_on`；使用当前 per-IRQ register API；先在独立 `examples/async_uart` 复现单端口 parity，再进入 stdio 和通用设备模型。
- **Impact**: 需要维护一个可修改的 platform fork/path patch，并为 `axtask` 增加少量 crate 内调度代码；PoC 明确限制单 reader/writer，但能最短路径验证 ArceOS 本体。
- **Alternatives**: 整体升级到 ArceOS 0.3 preview（范围和回归面过大）；照搬 StarryOS TTY/VFS（与 unikernel 目标无关）；先硬化 uart_16550 再集成（推迟已验证闭环的复现）。

<!-- A03 -->
### ADR-007: 设备 IRQ Gate 必须基于平台硬件契约并在 handler 清除设备中断源

- **Status**: Accepted
- **Date**: 2026-06-19
- **Context**: M1 RED probe 将 QEMU virt ns16550a 的 stride 误设为 4、在 IRQ 注册前执行越界 MMIO 写、未初始化 UART IER，且 handler 只计数不清除 RX level condition；这些夹具缺陷使 StoreFault 和后续 IRQ storm 都可能被误判为 PLIC 问题。
- **Decision**: 设备 IRQ probe 必须显式使用平台提供的 base/IRQ/stride 契约；注册 handler 后才开启设备中断；handler 必须确认并清除设备侧中断原因。RED/GREEN 仅允许控制器实现变化，设备配置与输入刺激保持一致。
- **Impact**: M1 probe 需要修正 stride、初始化顺序和 handler 行为；后续 NIC/Block IRQ Gate 也必须提供可重复刺激与设备侧 acknowledge，避免只验证 handler table。
- **Alternatives**: 仅用原子计数作为 handler（不能防 level IRQ storm）；手写少量寄存器绕过驱动（掩盖平台参数错误）；继续以越界 MMIO fault 调试 PLIC（因果路径不成立）。

<!-- A04 -->
### ADR-008: 应用通过顶层 feature 启用 IRQ 生命周期，不在平台或 probe 中直接开启全局中断

- **Status**: Accepted
- **Date**: 2026-06-19
- **Context**: M1 probe 直接启用 `axhal/irq` 后，平台成功设置 `sie.SEIE` 和 PLIC source/context，但 `axruntime/irq` 未启用，导致 `init_interrupt()` 与全局 `sstatus.SIE` enable 缺失。设备到 PLIC 已成立，却无法进入 S_EXT trap。
- **Decision**: ArceOS 应用需要 IRQ 时必须通过 `axstd/irq` 或 `axfeat/irq` 传播到 `axruntime/irq`；平台层只负责 per-hart interrupt enable 和控制器配置。probe 不得直接调用 `enable_irqs()` 绕过 runtime 生命周期。
- **Impact**: IRQ 测试夹具必须验证 Cargo feature graph 和 runtime 初始化日志；可确保 timer handler、全局 enable 与设备 IRQ 使用统一顺序。低层独立 crate 测试仍可显式启用 `axhal/irq`，但不能据此宣称完整 runtime IRQ 可用。
- **Alternatives**: 在 platform `init_later()` 直接设置 `sstatus.SIE`（开启时机过早且绕过 runtime handler 初始化）；在 probe main 中手工开启全局中断（测试不再代表真实应用配置）；让 `axhal/irq` 反向依赖 runtime（违反分层）。
