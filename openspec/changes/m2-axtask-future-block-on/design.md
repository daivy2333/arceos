## Context

当前 `axtask` 已具备构造此能力的所有内部原语：`CurrentTask` 可取得强引用，`current_run_queue().blocked_resched()` 可原子释放 guard 并阻塞当前任务，`select_run_queue().unblock_task()` 可从 Waker 恢复目标任务。CodeGraph impact 显示 `blocked_resched` 间接关联 60 个符号，包括 WaitQueue、Mutex、FS 和网络，因此实现必须完全复用现有状态转换，不新增第二套调度状态。

参考实现是 StarryOS 已使用的 `axtask 0.3.0-preview.2/src/future/mod.rs`，其 `AxWaker` 由 `WeakAxTaskRef` 和受 `SpinNoIrq<bool>` 保护的 `woke` 标志组成。

## Goals / Non-Goals

**Goals:**

- 提供最小、无 busy-loop 的单 Future `block_on`。
- 正确处理 poll/block 窗口早到 wake。
- 复用当前 axtask 的 SMP run queue 选择和 task 状态机。
- 为 M3 的两个 UART copier task 提供执行机制。

**Non-Goals:**

- 不提供多 Future executor、spawn(Future)、reactor 或 work stealing。
- 不提供 `poll_io`、IRQ waker map、timer、timeout、interruptible 或 cancellation。
- 不修改 WaitQueue、Mutex、scheduler 算法和公共 task 生命周期。
- 不在本 change 中接入 UART。

## Decisions

### 1. API 放在 `axtask::future::block_on`

新增 `modules/axtask/src/future/mod.rs`，仅在 multitask 实现导出：

```rust
pub fn block_on<F: IntoFuture>(future: F) -> F::Output
```

原因：该实现需要当前内核任务和 run queue 内部能力，放在外部 adapter 会迫使调度内部 API public；与已验证 0.3 API 保持一致可直接复用 StarryOS adapter。

备选：在 async UART adapter 自建 executor 会重复调度逻辑；引入 futures executor 会 busy-park 或依赖 std；整体升级 axtask 0.3 超出范围。

### 2. Waker 使用 task 弱引用

`AxWaker` 持有 `WeakAxTaskRef`，`block_on` 生命周期内另持有当前 task 强引用。wake 时先 upgrade；失败则安全返回。这样外部保存的 Waker 不延长已结束 task 生命周期。

### 3. 使用 woke handshake 防止丢唤醒

每轮 poll 前在 `SpinNoIrq<bool>` 下将 `woke=false`。Pending 后获取 current run queue，再持有同一个 woke guard：

```text
block_on task                         waking task/ISR
    | woke=false                           |
    | poll(Future) -> Pending              |
    |                                      | woke=true
    | lock woke                            | unblock_task
    | if false: blocked_resched(guard)     |
    | else: drop guard; yield_now          |
```

若 wake 先发生，`woke=true` 阻止 blocked_resched；若 block 先发生，wake 的 `unblock_task` 恢复任务。guard 同时承担检查与进入 wait queue 的同步边界。

备选：AtomicBool 无法直接作为 `blocked_resched` 所需 guard；无 handshake 会产生永久阻塞窗口；每次 Pending 仅 yield 会持续轮询。

### 4. wake 使用现有 run queue 选择

通过 `select_run_queue::<NoPreemptIrqSave>(&task).unblock_task(task, false)` 恢复任务，与当前 timer/WaitQueue 的 SMP 路径一致。Waker 不直接修改 TaskState。

### 5. TDD 测试分层

- 纯 Future：立即 Ready 和输出传递。
- 调度测试：一个任务 block_on Pending Future，另一任务设置条件并 wake。
- 竞态测试：测试 Future 在 Pending 路径内触发自身 Waker，强制覆盖早到 wake。
- 幂等测试：同轮多次 wake。
- 回归：现有 `test_wait_queue`、`test_task_join` 和 scheduler feature build。

RED 必须证明当前没有 `axtask::future::block_on` 或新测试无法编译；不得用测试永不结束作为 RED。

## Requirements Traceability Matrix

| Requirement | Task(s) | Coverage | Simplification | Status |
|-------------|---------|----------|----------------|--------|
| 单 Future 执行 | 1.1, 2.1, 3.1 | immediate/multi-poll tests | None | ✅ |
| Pending 真阻塞 | 2.2, 3.2, 4.2 | task state + peer progress | None | ✅ |
| 丢唤醒防护 | 2.2, 3.3 | deterministic self-wake race | None | ✅ |
| Waker 幂等与生命周期 | 2.3, 3.4 | duplicate/expired wake | None | ✅ |
| scheduler/SMP/feature 兼容 | 2.3, 4.1, 4.3 | feature build matrix + regressions | None | ✅ |
| 不引入完整 executor 能力 | 1.2, 4.4 | dependency/API review | 用户已批准范围边界 | ✅ |

## Risks / Trade-offs

- [共享调度原语回归] → 不改 `blocked_resched/unblock_task`，新增模块只调用它们；运行现有 axtask tests。
- [测试竞态偶发] → 自唤醒 Future 在 poll 内确定性触发早到 wake，不依赖 timing sleep。
- [重复 wake 重复入队] → 依赖 `unblock_task` 的状态转换保护，并添加多次 wake 测试。
- [中断上下文 wake 锁顺序] → 使用 `NoPreemptIrqSave` 与 `SpinNoIrq`，保持 0.3 已验证锁顺序。
- [API 被误认为通用 executor] → 文档明确一次只驱动一个 Future，不承诺公平性、取消或 IO reactor。

## Migration Plan

1. 先添加调用新 API 的测试并保存 compile-fail RED。
2. 增加 feature-gated `future` module 和最小 `AxWaker`。
3. 实现 block_on poll/handshake/block/unblock 循环。
4. 依次通过正确性、竞态、幂等和现有调度回归测试。
5. 完成 scheduler/SMP/non-multitask build matrix。

回滚：删除 `future` module/export 和新增测试。没有既有调用方、配置迁移或持久状态。

## Open Questions

无。公共路径、feature 边界、测试方法和排除能力均已固定。

