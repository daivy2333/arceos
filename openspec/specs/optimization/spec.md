## Purpose

记录 arceos 项目的性能瓶颈、代码异味、技术债、可优化点,持续跟踪并合理安排优化优先级。

> Version: 0.2.0  
> Last updated: 2026-06-19  
> Scope: 全项目(workspace)

## Requirements

### Requirement: 优化点记录

发现的性能问题、代码异味、技术债 SHALL 记录,含问题描述、当前影响、建议方案、优先级。

#### Scenario: 发现优化机会

- **WHEN** 开发者发现性能瓶颈、重复代码、过度复杂设计、可改进 API 等
- **THEN** 必须在本规范 Optimizations 表格追加一条,含上述四要素 + 状态(open/done/deferred)

#### Scenario: 评估优化价值

- **WHEN** 开发者评估某项优化是否值得做
- **THEN** 查阅本规范对应条目,参考当前影响与建议方案

### Requirement: 优化完成追踪

已完成的优化 SHALL 在原条目标注完成日期与实际效果,保留历史。

#### Scenario: 完成优化

- **WHEN** 开发者完成某项优化工作(commit 合入)
- **THEN** 在条目中追加 `Done: YYYY-MM-DD | 实际效果: ...`,状态置为 done

### Requirement: 优化优先级管理

优化点 SHALL 有优先级排序(P0/P1/P2),帮助安排优化顺序。

#### Scenario: 规划优化计划

- **WHEN** 制定下一阶段优化计划
- **THEN** 优先 P0,其次 P1,P2 视资源决定;P0 通常对应阻塞主线演进的核心瓶颈

## Optimizations

### P0 — 阻塞主线演进

| ID | 主题 | 模块 | 当前影响 | 建议方案 | 状态 |
|----|------|------|----------|----------|------|
| (暂无) | | | | | |

### P1 — 重要优化

| ID | 主题 | 模块 | 当前影响 | 建议方案 | 状态 |
|----|------|------|----------|----------|------|
| <!-- O02 --> | `block_current` preempt 断言缺失 | `axtask/run_queue` | 抢占式调度器下 `block_current` 未像 `blocked_resched` 一样检查 `can_preempt(2)`；SMP+preempt 场景可能导致未预期的上下文切换 | 添加 `#[cfg(feature = "preempt")] assert!(curr.can_preempt(2))` 到 `block_current`；参考 `blocked_resched` 第 404 行 | open |
| <!-- O04 --> | `percpu/sp-naive` + `preempt` 测试环境不兼容 — RR 调度器测试全部失败 | `axtask` 测试基础设施 | `sched-rr` 启用 `preempt` → `blocked_resched` 断言 `can_preempt(2)` 失败。根因：用户态测试的 `percpu/sp-naive` 无法追踪跨上下文切换的抢占计数。影响：`test_wait_queue`/`test_task_join` 断言失败，`test_sched_fifo`/`test_fp_state_switch` SIGABRT 崩溃。`block_on` 走 `block_current` 不受影响 | 方案 A：修复 `percpu/sp-naive` 支持 preempt 计数追踪；方案 B：sched-rr/cfs 测试走 QEMU 而非用户态 | open |

### P2 — 一般改进

| ID | 主题 | 模块 | 当前影响 | 建议方案 | 状态 |
|----|------|------|----------|----------|------|
| <!-- O03 --> | `block_on` 使用 `IntoFuture` 而非 `Future` trait bound | `axtask/future` | 当前 `block_on<F: Future>` 不接受直接 await-able 类型；`.await` 语法糖实际使用 `IntoFuture`。异步块同时实现两者，无功能影响 | 变更为 `block_on<F: IntoFuture>(future: F) -> F::Output`，internal pin 通过 `future.into_future()` | open |

## Performance Baseline

> 关键路径的性能基线,后续优化需对比基线证明改善。

| 场景 | 指标 | 当前值 | 测量方法 | 备注 |
|------|------|--------|----------|------|
| (暂无) | | | | |

## Notes

- 性能优化须有可复现的基准测试或真实负载验证,不可仅凭理论推断
- 优化前记录基线,优化后对比;无改善或退步的优化需回滚或重新评估
- 中断路径优化须严格保证中断安全(锁/禁用中断/内存屏障)
