## 1. RED 见证与 vendor 基线

- [ ] 1.1 [依赖: 无] 将 crates.io `axplat-riscv64-qemu-virt 0.4.1` 原样置于 `vendor/` 并配置 `[patch.crates-io]`；验收：未改逻辑时 RISC-V baseline build 与原依赖一致
- [ ] 1.2 [依赖: 1.1] 新增仅启用同步 UART API 的 `examples/uart_irq` probe；验收：输出可解析的 IRQ 计数与 enable 状态
- [ ] 1.3 [依赖: 1.2] Verify RED：在未实现 PLIC 时运行 probe；验收：保存 handler 未收到 IRQ 10 的失败证据和完整命令输出

## 2. PLIC 核心

- [ ] 2.1 [依赖: 1.3] 添加 `riscv_plic 0.2.0`、PLIC base 和 per-hart supervisor context 计算；验收：RISC-V 单核与 SMP 配置编译通过
- [ ] 2.2 [依赖: 2.1] 实现每 hart `init_percpu()` context 初始化；验收：BSP/AP context 测试或启动日志与预期 context 一致
- [ ] 2.3 [依赖: 2.2] 实现设备 source priority 与 enable/disable，显式忽略 IRQ 0；验收：IRQ 10 bit 状态场景通过
- [ ] 2.4 [依赖: 2.3] 实现 S_EXT claim、无锁 handler dispatch、complete 和 spurious 分支；验收：handler 执行期间不持有 PLIC lock

## 3. 生命周期与边界测试

- [ ] 3.1 [依赖: 2.4] Verify GREEN：运行 UART IRQ probe；验收：输入后 IRQ 10 计数增加且每次 claim 被 complete
- [ ] 3.2 [依赖: 3.1] 验证 disable/re-enable；验收：disable 区间计数不变，重新 enable 后恢复
- [ ] 3.3 [依赖: 3.1] 验证重复注册、unregister、未注册 IRQ 与 spurious trap；验收：无 panic、无 IRQ storm、原 handler 不被覆盖

## 4. Gate 与回滚

- [ ] 4.1 [依赖: 3.2, 3.3] 运行 RISC-V 默认启动回归；验收：SBI console 在 PLIC 初始化前后均可用
- [ ] 4.2 [依赖: 4.1] 连续运行 IRQ probe 并输入 1B/64B；验收：计数匹配且无 unhandled IRQ 0
- [ ] 4.3 [依赖: 4.2] 运行无输入超时场景；验收：无 spurious loop，QEMU 空闲稳定
- [ ] 4.4 [依赖: 4.1] 运行现有非 RISC-V build matrix；验收：无新增编译回归
- [ ] 4.5 [依赖: 4.2, 4.3, 4.4] 执行 spec compliance 与 code quality 两阶段 review，并演练移除 `[patch.crates-io]` 的回滚步骤；验收：无 Critical/Important issue，回滚命令可复现

