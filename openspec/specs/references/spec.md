## Purpose

记录 arceos 项目的依赖、跨项目关系、外部参考资源与文档索引,确保依赖可追溯、外部资源可获取。

> Version: 0.1.0  
> Last updated: 2026-06-19  
> Scope: 全项目(workspace)

## Requirements

### Requirement: 依赖版本锁定

所有 arceos 模块/用户态库/示例中的外部依赖 SHALL 在 Cargo.toml 中锁定或浮动版本可控,版本变更须在本规范记录。

#### Scenario: 添加新依赖

- **WHEN** 开发者引入新的 crate 依赖(如某 async runtime、某设备协议栈)
- **THEN** 必须在本规范 Dependencies 表格登记:依赖名、版本、来源、用途

#### Scenario: 更新依赖版本

- **WHEN** 开发者升级或降级某个依赖
- **THEN** 须更新本规范对应条目,标注变更日期与原因

### Requirement: 外部资源记录

arceos 开发过程中需要参考的外部文档、规范、数据手册 SHALL 在本规范登记。

#### Scenario: 查阅规范

- **WHEN** 开发者需要查阅 16550 UART 规范、smoltcp 接口、QEMU virt 设备列表等
- **THEN** 在 External Resources 表格定位,直接访问链接

### Requirement: 跨子项目索引

arceos 工作区可能包含子项目(如 StarryOS 上层、依赖的第三方 crate),本规范 SHALL 登记子项目关系与文档索引路径。

#### Scenario: 跨项目定位

- **WHEN** 开发者需要查阅上层文档(如上层工作区 CLAUDE.md、StarryOS 文档、uart_16550 文档)
- **THEN** 在 Cross-Project Index 区查找相对路径

## Dependencies

> 来源:`Cargo.toml` workspace.dependencies + 常用外部 crate。版本策略:workspace 统一或 path 引用。

### Workspace 内部成员(全部 path 引用)

| 模块 | 路径 | 角色 |
|------|------|------|
| axstd | `ulib/axstd` | Rust 用户态 std 替代 |
| axlibc | `ulib/axlibc` | C 标准库 |
| arceos_api | `api/arceos_api` | 系统调用暴露 |
| arceos_posix_api | `api/arceos_posix_api` | POSIX 兼容层 |
| axfeat | `api/axfeat` | 用户态 feature 暴露 |
| axalloc | `modules/axalloc` | 内核堆分配 |
| axconfig | `modules/axconfig` | 编译期配置(由 axconfig-gen 生成) |
| axdisplay | `modules/axdisplay` | 显示/GPU |
| axdriver | `modules/axdriver` | 设备驱动集合 |
| axfs | `modules/axfs` | 文件系统 |
| axhal | `modules/axhal` | 硬件抽象层 |
| axlog | `modules/axlog` | 内核日志 |
| axmm | `modules/axmm` | 内存管理 |
| axnet | `modules/axnet` | 网络栈 |
| axns | `modules/axns` | 命名空间 |
| axruntime | `modules/axruntime` | 内核运行时/初始化 |
| axsync | `modules/axsync` | 同步原语 |
| axtask | `modules/axtask` | 任务调度 |
| axdma | `modules/axdma` | DMA |
| axipi | `modules/axipi` | 核间中断 |

### 常用外部 crate(随项目演进)

| crate | 典型用途 | 备注 |
|-------|----------|------|
| `smoltcp` | TCP/UDP 网络栈(README 引用) | 已是 arceos 标准网络栈 |
| `axconfig-gen`(工具) | 从 TOML 生成 axconfig crate | `cargo install axconfig-gen` |
| `cargo-binutils` | rust-objcopy/rust-objdump | `cargo install cargo-binutils` |
| `cargo-axplat` | platform crate 生成 | `cargo install cargo-axplat` |
| `musl-cross` | C 工具链 | 见 README QEMU 下载章节 |
| `mkfs.fat`(`dosfstools`) | `make disk_img` 生成 fat32 镜像 | Ubuntu: `sudo apt install dosfstools` |

## External Resources

| 资源 | 链接 | 用途 |
|------|------|------|
| arceos 主仓库 | https://github.com/arceos-org/arceos | 源代码 |
| arceos 文档站 | https://arceos-org.github.io/arceos/ | API/教程 |
| Unikraft | https://github.com/unikraft/unikraft | 设计灵感来源 |
| smoltcp | https://github.com/smoltcp-rs/smoltcp | 网络栈 |
| axconfig-gen | https://github.com/arceos-org/axconfig-gen | 配置生成工具 |
| cargo-axplat | https://github.com/arceos-org/axplat_crates | platform 生成 |
| arceos-apps | https://github.com/arceos-org/arceos-apps | 应用示例仓库 |
| 16550 UART 规范 | (learned.md 中记录具体链接) | UART 寄存器定义 |
| QEMU 下载 | https://www.qemu.org/download/ | 多架构 QEMU |
| dosfstools(mkfs.fat) | https://github.com/dosfstools/dosfstools | fat32 格式化工具,`make disk_img` 必需 |

## Cross-Project Index

> 上层工作区 `/home/daivy/projects/serial/` 包含 arceos 及可能的关联子项目。

| 上层路径 | 角色 | 文档 |
|----------|------|------|
| `../CLAUDE.md` | 跨项目文档索引 | 双子项目协作总览 |
| `../StarryOS/`(可选) | StarryOS 子项目 | 自身 CLAUDE.md + openspec |
| `../uart_16550/`(可选) | uart_16550 驱动库 | 自身 CLAUDE.md |

> 注:这些子项目若存在,由其各自的 OpenSpec 体系管理,本规范只登记路径,不做内容迁移。

## 子项目索引

<!-- 由 openspec-liaison 写入，由 openspec-assistant 日常维护，由 openspec-archivist 周期清理。 -->
<!-- 添加时格式: <!-- R{编号} --> | 子项目 | 路径 | 文档体系 | 摘要 | 最近更新 | -->

<!-- R01 --> | uart_16550 | ../uart_16550 | 传统 SNAPSHOT✓ tasks✓ learned✗(只有 .bak) arch✗(只有 .bak) refs✗(只有 .bak) | OpenSpec config✓ specs✓ changes✗(仅 archive) | cg✓ | 16550 UART 驱动库 v0.6.0,no_std 嵌入式驱动库,提供同步+异步 UART 完整栈 (ISR + ring buffer + copier + device_ops + 5 OS 抽象 trait),feat/uart-16550-async 分支领先 main 21 commits,作为 StarryOS 串口底层模块 | 2026-06-17 |

## Project Analysis Docs

> openspec-explorer 生成的深度分析文档索引。

| 主题 | 路径 | 内容概要 |
|------|------|----------|
<!-- R02 --> | ArceOS 当前串口与控制台设计 | `.claude/analysis/arceos-serial-design.md` | RISC-V SBI/x86 16550 后端、stdio 轮询链路、启动顺序与 PLIC 缺口 |
<!-- R03 --> | uart_16550 异步串口集成方案 | `.claude/analysis/uart16550-async-integration.md` | ISR/copy task/ring/Future 机制审计、风险清单与分阶段 ArceOS 集成方案 |
<!-- R04 --> | StarryOS 到 ArceOS 异步 UART 迁移路线 | `.claude/analysis/starry-to-arceos-async-uart-roadmap.md` | 已验证链路、版本能力差异、最小回移方案与 M0-M6 验收里程碑 |
<!-- R05 --> | M1 UART IRQ probe 阻塞分析 | `.claude/analysis/m1-uart-irq-probe-blocker.md` | StoreFault 因果定位、stride/init/IRQ 清源缺陷及 RED/GREEN 解阻方案 |
<!-- R06 --> | M1 S_EXT feature gate 阻塞分析 | `.claude/analysis/m1-sext-feature-gate-blocker.md` | `axruntime/irq` 缺失导致 `sstatus.SIE=0` 的证据链、最小修复与分阶段验证方案 |

> 五份文档互相交叉引用，覆盖当前实现、驱动审计、迁移路线和 M1 两阶段阻塞解法。
