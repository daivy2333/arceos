# Tasks — arceos 全局任务追踪

> Last updated: 2026-06-19 (M3 7/7 ✅ — async_uart parity build passes, M4 ready)
> 与 `openspec/changes/` 双向同步:每个进行中的 change 都有对应 Task 编号

## Milestone 路线

| Milestone | 目标 | 依赖 | Gate | 状态 |
|-----------|------|------|------|------|
| M0 | 当前 ArceOS、uart_16550、StarryOS 已验证实现的证据链与迁移设计 | — | 三份分析文档 + ADR-005/006 | ✅ 完成 |
| M1 | RISC-V PLIC 与 UART IRQ 10 基线 | M0 | claim/complete、enable/disable、无 IRQ storm | ✅ 完成（15/15 GREEN Gate PASS） |
| M2 | axtask 单 Future `block_on` 基线 | M0 | Pending 真阻塞、wake 恢复、竞态不丢唤醒 | ✅ 完成（14/14 GREEN：10 tests pass + spec compliance + rollback） |
| M3 | `examples/async_uart` 复现 StarryOS parity | M1 + M2 | RX/TX copier + ring 双向 echo | ✅ 完成 (7/7 GREEN) |
| M4 | QEMU 稳定性 Gate | M3 | 10 分钟压力、20 次启动、空闲无轮询 | 待办 |
| M5 | ArceOS stdin/readiness/stdout 分阶段接入 | M4 | shell 输入无 yield-poll，SBI fallback 保留 | 待办 |
| M6 | 驱动硬化与通用 bottom-half 模式 | M5 | SMP/并发契约、测试、性能基线 | 待办 |

关键路径：`M1 + M2 -> M3 -> M4 -> M5 -> M6`。M4 前不引入 TTY、POSIX 或通用设备模型扩展。

## 进行中(In Progress)

| ID | 主题 | 分支 | 关联 Change | 状态 |
|----|------|------|-------------|------|
| — | — | — | — | — |

## 阻塞(Blocked)

| ID | 主题 | 阻塞原因 | 解阻条件 |
|----|------|----------|----------|
| — | — | — | — |

## 待办(Backlog)

| ID | 主题 | 优先级 | 来源 | 备注 |
|----|------|--------|------|------|
<!-- T-008 --> | PLIC 可修改载体与移植方案 | P0 | change `m1-riscv-plic-baseline` | vendor 0.4.1 + `[patch.crates-io]` 方案已定 |
<!-- T-009 --> | 移植并验证 RISC-V PLIC | P0 | change `m1-riscv-plic-baseline` | 15 个任务；UART IRQ 10 RED/GREEN Gate |
<!-- T-010 --> | axtask 最小 Future `block_on` 回移 | P0 | change `m2-axtask-future-block-on` | 仅单 Future executor，不引入 axpoll | ✅ 完成 |
<!-- T-011 --> | Future 调度竞态测试 | P0 | change `m2-axtask-future-block-on` | 14 个任务；含确定性 self-wake 场景 | ✅ 完成 |
<!-- T-012 --> | 创建 `examples/async_uart` 与依赖特性 | P0 | M3 | ✅ 完成 (T-012) |
<!-- T-013 --> | 移植最小 ArceOS UART adapter/bootstrap | P0 | M3 | ✅ 完成 (T-013) |
<!-- T-014 --> | 异步 RX/TX echo 闭环 | P0 | M3 | ✅ 完成 (T-014) |
<!-- T-015 --> | QEMU 稳定性与空闲验证 | P0 | M4 | 1B/64B/4KiB、10 分钟、20 次启动、无 busy-loop |
<!-- T-016 --> | UART 可观测性计数 | P1 | M4 | IRQ/wake/drop/ring high-water mark |
<!-- T-017 --> | stdin 改为 RX ring + 阻塞唤醒 | P1 | M5 | 删除 yield-poll，保持 CR/LF 行为 |
<!-- T-018 --> | stdout/readiness 分阶段接入 | P1 | M5 | readiness 真实化；SBI early/panic fallback |
<!-- T-019 --> | uart_16550 并发与 async 语义硬化 | P1 | M6 | SPSC 所有权、per-port waker、IER 原子、read/flush |
<!-- T-020 --> | ArceOS/StarryOS 性能回归基线 | P1 | M6 | 以 Q13 129.5µs 为参考，不直接套用 QEMU 吞吐数字 |
<!-- T-005 --> | 中断驱动设备 I/O 通用模式沉淀 | P1 | README TODO / M6 | UART Gate 后提炼到 NIC/Block |
<!-- T-007 --> | 完善 axdriver 中断 bottom-half 模式 | P1 | async-uart-1 / M6 | parity 完成后决定 axdriver::uart 或独立 axuart |
<!-- T-006 --> | Linux 应用兼容 | P2 | README TODO | 远期，不进入当前关键路径 |

## 已完成(Done — 历史归档)

| ID | 主题 | 完成日期 | 备注 |
|----|------|----------|------|
| D-001 | OpenSpec + CodeGraph 初始化 | 2026-06-15 | openspec 1.4.0 / codegraph 0.9.9 / 4 spec / healthy 索引 |
| D-002 | 分支 async-uart-1 创建 | 2026-06-15 | 基于 main,等待首次 commit |
| D-003 | examples/shell riscv64 启动验证 | 2026-06-17 | 配方 `BLK=y APP_FEATURES=use-ramfs`; 串口透传基线已通; 3 条踩坑 P01/P02/P03 写入 learned |
<!-- T-001 --> | 异步串口设计与迁移路线 | 2026-06-19 | 完成 ArceOS/uart_16550/StarryOS 三方分析，形成 M0-M6 |
<!-- T-002 --> | 当前串口实现审计 | 2026-06-19 | 确认 RISC-V console 走 SBI、x86 走同步 16550、无 axdriver::uart |
<!-- T-003 --> | axhal/axplat 中断链审计 | 2026-06-19 | 确认当前 platform 0.4.1 PLIC TODO，并定位 0.3.1-pre.6 可移植实现 |
<!-- T-004 --> | axtask Future 唤醒点调研 | 2026-06-19 | 确认 0.2 调度原语可承载 0.3 单 Future block_on 回移 |
| D-004 | StarryOS 已验证路径反向迁移分析 | 2026-06-19 | QEMU 已验证链路、版本差异、最小能力回移方案已记录 |
| **M1** | RISC-V PLIC & UART IRQ 10 baseline (15/15) | 2026-06-19 | GREEN Gate PASS: 10B→count=10, 64B→count=64; SBI console/idle/build matrix OK |
| **M1-T1.3** | uart_irq probe RED 验证 | 2026-06-19 | 修复 4 处缺陷（stride 4→1、删除越界诊断写、添加 `uart.init(Config::default())`、handler drain RBR）；6s 内 519 heartbeats `count=0 rx=0`，无 StoreFault/panic；日志 `/tmp/m1-t1-3-red.log` |
| **M2** | axtask future block_on 基线 (14/14) | 2026-06-19 | block_on + AxWaker + woke handshake + 6 tests GREEN + spec compliance + rollback；修复丢失唤醒竞态窗口；RR 测试预存问题诊断记录 |
| **M3** | async UART parity (7/7) | 2026-06-19 | examples/async_uart: ArceOsRuntime + ArceOsWakerSet + UartPort adapter + RingBuffer + IRQ trampoline + bootstrap + ring-aware echo Future. Build GREEN. M1 regression ✅. Rollback: delete workspace member + dir. M4 ready. |

## 与 OpenSpec changes/ 同步说明

- 每个 `openspec/changes/<name>/` 提案对应一条 Backlog Task
- Proposal 通过 `/opsx:propose` 创建,验证用 `openspec validate <name>`
- Tasks 完成对应 `/opsx:apply` 与 commit
- 归档用 `/opsx:archive`,对应任务移到 Done 区

## Task 添加规则

- P0:阻塞当前主线演进,必须本周完成
- P1:重要但非阻塞,本月完成
- P2:改进项,资源允许时安排
- 任务添加需含:主题、分支、关联 Change、优先级、来源

## 快速链接

- 架构规范:`openspec/specs/architecture/spec.md`
- 学习记忆:`openspec/specs/learned/spec.md`
- 外部参考:`openspec/specs/references/spec.md`
- 优化记录:`openspec/specs/optimization/spec.md`
- 项目快照:`SNAPSHOT.md`
- OpenSpec CLI:`openspec list` / `openspec validate --specs`
- 实施路线:`../analysis/starry-to-arceos-async-uart-roadmap.md`
- M1 阻塞分析:`../analysis/m1-uart-irq-probe-blocker.md`
