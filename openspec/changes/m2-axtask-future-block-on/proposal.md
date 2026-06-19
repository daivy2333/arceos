## Why

`uart_16550` 的 RX/TX copier 是长期 Pending 的 Future，而当前 ArceOS `axtask 0.2` 只能运行 `FnOnce`，无法在无轮询的条件下驱动它们。StarryOS 已验证 `axtask 0.3` 的单 Future `block_on` 模式，当前需要最小回移该能力，为 M3 提供调度基础。

## What Changes

- 在 `axtask` multitask 实现中新增 `future` 模块和公开 `block_on(Future)` API。
- 使用当前 task/run queue 的 block/unblock 原语实现 task-backed Waker。
- 使用 wake handshake 消除 Future 返回 Pending 与任务阻塞之间的丢唤醒窗口。
- 增加立即 Ready、Pending/wake、早到 wake、重复 wake 和 SMP 编译场景测试。
- 不引入 `axpoll`、通用 executor、任务队列、timer Future、`poll_io`、timeout 或 cancellation。

## Capabilities

### New Capabilities

- `axtask-future-block-on`: 在当前内核任务上同步驱动单个 Future，并在 Pending 时阻塞、wake 时恢复。

### Modified Capabilities

无。

## Impact

- 主要代码：`modules/axtask/src/lib.rs`、新增 `future/mod.rs`、`modules/axtask/src/tests.rs`。
- 调度内部：复用 `current_run_queue`、`select_run_queue`、`blocked_resched`、`unblock_task`；不改变既有 API 语义。
- 间接风险：WaitQueue、Mutex、FS、网络等共享相同 block/unblock 原语，必须运行现有 axtask 回归测试。
- feature：API 仅在 `multitask` 实现存在；dummy/non-multitask 配置不暴露伪阻塞实现。
- 回滚：删除 `future` module/export 和新增测试即可，无持久数据或公共调用方迁移。

