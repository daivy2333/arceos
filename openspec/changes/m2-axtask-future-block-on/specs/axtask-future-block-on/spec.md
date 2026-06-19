# axtask-future-block-on Delta Specification

> Version: 0.1.0
> Last updated: 2026-06-19

## ADDED Requirements

### Requirement: 单 Future 执行

`axtask::future::block_on` SHALL 在当前已初始化的内核任务上持续 poll 一个 Future，直到返回 Ready，并返回其输出。

#### Scenario: Future 立即 Ready
- **WHEN** 当前任务调用 `block_on` 且 Future 首次 poll 返回 Ready
- **THEN** `block_on` 立即返回输出，任务不进入 Blocked 状态且不触发额外调度

#### Scenario: 多次 Pending 后 Ready
- **WHEN** Future 经多次 wake 后才返回 Ready
- **THEN** `block_on` 保持同一 Future 实例和输出所有权，最终只返回一次结果

### Requirement: Pending 时阻塞当前任务

Future 返回 Pending 且当前 poll 周期未发生 wake 时，`block_on` MUST 将当前任务置为 Blocked 并触发 reschedule，不得使用 busy-loop 或 `yield_now` 轮询等待。

#### Scenario: 无 wake 的 Pending
- **WHEN** Future 返回 Pending 且没有 waker 被触发
- **THEN** 当前任务进入 Blocked，其他 Ready 任务能够运行

#### Scenario: 外部任务 wake
- **WHEN** 当前任务因 Future Pending 而 Blocked，另一任务调用其 Waker
- **THEN** 原任务转换为 Ready 并在调度后继续 poll Future

### Requirement: 丢唤醒防护

`block_on` MUST 使用原子化的 wake handshake，覆盖 poll 返回 Pending 到 blocked reschedule 之间的 wake 窗口。

#### Scenario: 早到 wake
- **WHEN** Waker 在 Future 返回 Pending 后、当前任务正式 Blocked 前触发
- **THEN** 当前任务不得永久阻塞，并在有限调度步骤内再次 poll Future

#### Scenario: poll 前遗留 wake
- **WHEN** 上一轮 wake 标记已消费并开始下一轮 poll
- **THEN** `block_on` 清晰地区分新旧 wake，不把旧标记当成当前轮唤醒

### Requirement: Waker 幂等与任务生命周期

Future Waker SHALL 只持有目标任务的弱引用；重复或过期 wake MUST 不造成重复入队、use-after-free 或 panic。

#### Scenario: 重复 wake
- **WHEN** 同一 poll 周期对同一 Waker 连续调用多次 wake
- **THEN** 目标任务保持可调度且不会以多个 Ready 实例重复入队

#### Scenario: 任务已退出
- **WHEN** Waker 在目标任务已不可升级后触发
- **THEN** wake 安全返回且不访问已释放任务

### Requirement: feature 与跨架构兼容

Future 支持 SHALL 复用 `axtask` 现有 multitask、SMP 和 scheduler 抽象，不得引入架构特定代码。

#### Scenario: multitask 构建
- **WHEN** 以 FIFO、RR 或 CFS 任一现有 scheduler 构建 multitask 配置
- **THEN** `axtask::future::block_on` 可用且不要求 `axpoll`

#### Scenario: SMP 构建
- **WHEN** 启用 SMP 且其他 CPU 触发 Waker
- **THEN** 系统通过目标任务 CPU mask 选择 run queue 并安全 unblock

#### Scenario: non-multitask 构建
- **WHEN** 构建不启用 multitask 的 ArceOS 配置
- **THEN** 不编译 task-backed `block_on`，既有 dummy task API 保持不变

