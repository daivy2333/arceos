## 1. RED 见证与 API 边界

- [ ] 1.1 [依赖: 无] 添加引用 `axtask::future::block_on` 的最小立即 Ready 测试；验收：当前代码因 API 不存在而 compile-fail，并保存完整 RED 输出
- [ ] 1.2 [依赖: 1.1] 添加 compile-time API 边界检查；验收：计划中不出现 `poll_io`、timeout、interruptible、spawn(Future) 或 `axpoll` 新依赖

## 2. 最小实现

- [ ] 2.1 [依赖: 1.1] 新增 multitask-only `future` module、`AxWaker` 和 `block_on` 基础 poll 循环；验收：立即 Ready 测试 GREEN 且不触发 reschedule
- [ ] 2.2 [依赖: 2.1] 实现 `woke` handshake、Pending 阻塞和 wake 恢复；验收：Pending 任务进入 Blocked，peer task 继续运行并可唤醒
- [ ] 2.3 [依赖: 2.2] 使用 `WeakAxTaskRef` 与现有 run queue selector 完成幂等 wake；验收：重复 wake 不重复入队，过期 weak ref 安全返回

## 3. 调度与竞态测试

- [ ] 3.1 [依赖: 2.1] 添加多次 Pending 后 Ready 与输出所有权测试；验收：poll 次数和最终输出精确匹配
- [ ] 3.2 [依赖: 2.2] 添加双任务 block/wake 测试；验收：等待任务 Blocked 期间 peer task 有进展，唤醒后完成
- [ ] 3.3 [依赖: 2.2] 添加 Future poll 内 self-wake 的确定性早到 wake 测试；验收：不永久 Blocked，并在有限 poll 次数内 Ready
- [ ] 3.4 [依赖: 2.3] 添加重复 wake 与 Waker 生命周期测试；验收：无 panic、无重复 Ready task、无强引用泄漏

## 4. 回归与 Gate

- [ ] 4.1 [依赖: 3.1, 3.2, 3.3, 3.4] 运行 axtask 全量测试；验收：新增测试与现有 WaitQueue/task join 测试全部通过
- [ ] 4.2 [依赖: 4.1] 记录 Pending 等待期间的调度证据；验收：无 `yield_now` busy-loop，peer task 能持续运行
- [ ] 4.3 [依赖: 4.1] 运行 FIFO/RR/CFS、SMP 和 non-multitask build matrix；验收：multitask API 可用，non-multitask 不编译 task-backed module，其他架构无特定代码
- [ ] 4.4 [依赖: 4.2, 4.3] 执行 spec compliance 与 code quality 两阶段 review；验收：无 Critical/Important issue，未引入计划外 executor 能力或依赖
- [ ] 4.5 [依赖: 4.4] 演练删除 module/export 的回滚并恢复；验收：回滚后原 axtask tests 与 baseline build 可复现

