## Purpose

记录 arceos 开发过程中的学习记忆(API 路径、踩坑档案、技巧模式、文件速查),避免重复探索,加速问题定位与决策。

> Version: 0.2.3  
> Last updated: 2026-06-19 (M2 验证收尾: L18 丢失唤醒 + L19 RR 测试根因诊断 + optimization O04)  
> Scope: 全项目(workspace)

## Requirements

### Requirement: API 路径记录

所有 arceos 内核/驱动/API/示例中需要反复使用的关键 API 调用 SHALL 记录,含用途、模块路径、调用示例。

#### Scenario: 查找 API 用法

- **WHEN** 开发者不确定如何调用某个内核能力(注册中断、分配页、唤醒任务等)
- **THEN** 应在本文 API 路径区定位到对应条目,或通过 grep 模块路径快速定位源码

#### Scenario: 记录新 API 模式

- **WHEN** 开发者发现新的常用 API 调用模式(如 spin_lock_irqsave + 临界区 + spin_unlock_irqrestore)
- **THEN** 必须将模式写入 API 路径区,标注模块、典型用法、注意事项

### Requirement: 踩坑经验记录

arceos 内核开发中的技术陷阱、根因、解决方案、预防 SHALL 记录,防止重复踩坑。

#### Scenario: 排查新问题前

- **WHEN** 开发者遇到编译错误、运行崩溃、中断未触发、SMP 死锁等异常
- **THEN** 应先 grep 踩坑档案关键词(中断、安全、锁、SMP、QEMU 等)确认是否已有记录

#### Scenario: 解决棘手问题

- **WHEN** 开发者花费 ≥10 分钟解决一个非显然问题
- **THEN** 必须新增一条踩坑档案条目,包含:症状、根因、解决方案、预防措施

### Requirement: 技巧模式记录

arceos 特有的高效开发技巧与模式 SHALL 记录,促进知识共享。

#### Scenario: 发现新技巧

- **WHEN** 开发者发现一种高效的 arceos 开发模式(如用 axconfig-gen 生成配置、QEMU 调试断点)
- **THEN** 必须记录到技巧模式区,含技巧名称、适用场景、使用方法

### Requirement: 文件速查表

arceos 中需要反复访问的关键文件/目录 SHALL 记录,加速代码导航。

#### Scenario: 定位关键代码

- **WHEN** 开发者需要找到某个文件(如中断向量表、调度器入口)
- **THEN** 通过文件速查表直接定位

## API 路径速查

### 内核初始化

- **入口**: `axhal/src/lib.rs` 中 `rust_main`(由各架构的 `_start` 跳转)
- **模块装载**: `axruntime::rust_main`(modules/axruntime/src/lib.rs)按顺序初始化各子系统

### 中断子系统

- **注册/注销**: `axhal::irq::register_handler(irq_num, handler)`
- **开关中断**: `axhal::irq::enable_irq(irq_num)` / `disable_irq`
- **中断安全锁**: `axsync::SpinLock::lock_irqsave()`(自动 save/restore 中断状态)

### 任务调度

- **创建任务**: `axtask::spawn(future, name)`
- **让出/唤醒**: `axtask::yield_now()` / `axtask::wake_task(id)`
- **运行时**: `axruntime` 维护 idle 任务、主调度循环

### 驱动接口

- **UART 输出**: `axdriver::uart::Uart::put_byte` / `put_bytes`(early console 走 `axlog`)
- **块设备**: `axdriver::block::BlockDevice::read_block` / `write_block`

### 用户态入口

- **axstd**: `ulib/axstd`(Rust 风格 std 替代)
- **axlibc**: `ulib/axlibc`(C 标准库)
- **arceos_api**: `api/arceos_api`(OS 内部 syscall 暴露)

## 踩坑档案

### P01: axfs 启动需要块设备,`use-ramfs` 也救不了

- **症状**: 启动 `examples/shell` 时 panic
  ```
  panicked at modules/axfs/src/lib.rs:42:35
  No block device found!
  ```
- **根因**:
  - `Makefile` 默认 `BLK=n`,QEMU 不会加 `-device virtio-blk-pci`
  - 即便用户认为 `APP_FEATURES=use-ramfs` 能避开块设备(因为 ramfs 不需要磁盘),实际不行
  - 原因在 `axfs/src/root.rs:149-158`:`init_rootfs(disk)` 强制接收 `Disk` 对象,然后再调 `MyFileSystemIf::new_myfs(disk)`
  - ramfs 实现 (`examples/shell/src/ramfs.rs:12`) 写的是 `fn new_myfs(_disk: AxDisk)`,**收下但忽略**——但调用方仍然需要先有 `Disk`
- **解决方案**: 三件套缺一不可
  ```bash
  # 1. 一次性生成占位 fat32 镜像 (依赖 mkfs.fat / dosfstools)
  make A=examples/shell ARCH=riscv64 disk_img

  # 2. 启动 (同时启 BLK + APP_FEATURES=use-ramfs)
  make A=examples/shell ARCH=riscv64 BLK=y APP_FEATURES=use-ramfs run
  ```
- **预防**:
  - 看到 `No block device found!` 第一反应:**先加 `BLK=y` + 检查 `disk.img` 是否存在**,再去想 feature
  - 不要被 `use-ramfs` 字面意思误导,它**不是** rootfs 替代方案,而是 rootfs 的**挂载层**(`/tmp`)
  - 想看完整启动流程,用 `LOG=info` 跑一次
- **关联**: `modules/axfs/src/lib.rs:42`, `modules/axfs/src/root.rs:149-178`, `examples/shell/src/ramfs.rs`

### P02: `use-ramfs` 是 app feature,不是 axstd feature

- **症状**: cargo 报错
  ```
  package `arceos-shell` depends on `axstd` with feature `use-ramfs`
  but `axstd` does not have that feature.
  failed to select a version for `axstd`
  ```
- **根因**:
  - `Makefile` 的 `FEATURES=...` 会**自动加 `axstd/` 前缀**(看构建命令 `cargo ... --features "axstd/defplat axstd/log-level-warn axstd/use-ramfs"`)
  - `use-ramfs` 定义在 `examples/shell/Cargo.toml:10`,**不是** axstd 的 feature
  - 用 `FEATURES=use-ramfs` → cargo 解析 `axstd/use-ramfs` → axstd 没这个 feature → 报错
- **解决方案**: 用 `APP_FEATURES=`(Makefile 第 21 行有专门给应用的变量)
  ```bash
  # 错
  make A=examples/shell ARCH=riscv64 FEATURES=use-ramfs run

  # 对
  make A=examples/shell ARCH=riscv64 APP_FEATURES=use-ramfs run
  ```
- **预防**:
  - 参数分工:`FEATURES=xxx` → 自动加 `axstd/` 前缀 → axstd 内核 feature;`APP_FEATURES=xxx` → 应用层 feature
  - 凡是 feature 定义在 `examples/<app>/Cargo.toml` 而不是 `ulib/axstd/Cargo.toml` 的,一律走 `APP_FEATURES`
- **关联**: `Makefile:20-21`, `examples/shell/Cargo.toml:10`

### P03: `make disk_img` 依赖 `mkfs.fat`,Ubuntu 默认未装

- **症状**:
  ```
  Creating FAT32 disk image "disk.img" ...
  64+0 records in
  64+0 records out
  67108864 bytes (67 MB, 64 MiB) copied, ...
  make: mkfs.fat: No such file or directory
  make: *** [Makefile:227: disk_img] Error 127
  ```
- **根因**:
  - `dd` 写出 64MB 空白文件成功,但 `mkfs.fat` 格式化步骤找不到
  - `mkfs.fat` 在 `dosfstools` 包里,Ubuntu/Debian 默认不预装
- **解决方案**:
  ```bash
  sudo apt install dosfstools    # 提供 mkfs.fat
  # 或 macOS
  brew install dosfstools
  ```
- **预防**: 新机器装环境时一次性装齐:`qemu-system` + `dosfstools` + `musl-tools`
- **关联**: `Makefile:223-228`, `scripts/make/utils.mk:15-22`

### 6. RISC-V MMIO 调试: 读 satp + 走 page table + 读 scause/stval

- **场景**: RISC-V 平台 StoreFault/LoadFault 时快速定位是 page table、PMP 还是 device 行为
- **方法**:
  ```rust
  // 1. 读 satp (RV64: MODE[63:60] | ASID[59:44] | PPN[43:0])
  let satp: usize;
  unsafe { core::arch::asm!("csrr {0}, satp", out(reg) satp); }
  let mode = (satp >> 60) & 0xf;     // 8 = Sv39
  let ppn = satp & ((1usize << 44) - 1);
  let root_pa = ppn << 12;
  // 2. 走 page table: VA → VPN[2] = (VA >> 30) & 0x1ff → 索引 L2
  //    用 phys_to_virt(root_pa).as_usize() as *const u64 读 PTE
  // 3. 写之前先 csrr scause/stval/sscratch 确认 CPU 干净
  let scause: usize; let stval: usize; let sscratch: usize;
  unsafe { core::arch::asm!(
      "csrr {0}, scause", "csrr {1}, stval", "csrr {2}, sscratch",
      out(reg) scause, out(reg) stval, out(reg) sscratch,
  ); }
  ```
- **判断矩阵**:
  | scause | 含义 | 排查方向 |
  |--------|------|----------|
  | 1 (Instr access fault) | 取指 fault | 文本段 PTE / 段对齐 |
  | 5 (Load access fault) | load fault | page table / PMP / device 越界 |
  | 7 (Store/AMO access fault) | store fault | page table W=0 / PMP / device 越界 |
  | 8 (EnvCall) | ecall | 不是 fault |
- **关联**: 排查 P04 时的工具链

### P04: QEMU RISC-V 16550 仅有 8 个字节寄存器，错误 stride/越界诊断写会触发 StoreFault

- **症状**: 在 `examples/uart_irq` probe 中,直接 MMIO 写 UART 偏移 8 触发 kernel panic
  ```
  [uart_irq] write plic off=0x210000 ok       # 任意 PLIC 偏移都 OK
  [uart_irq] write uart off=0x1000 ok         # 4K 边界 OK
  [panic] Unhandled trap Exception(StoreFault) @ sb s1, 8(s2)
  ```
  注:偏移 0..7 是合法设备寄存器；偏移 8 已越过 ns16550a 设备窗口。该 fault 发生在 IRQ 注册和 PLIC enable 之前。
- **根因(已确认)**:
  - 1G page table entry 正确(`L2[0x100]=0xef V=R=W=X=1`)
  - probe 在 `register(10)` 前显式执行 `write_volatile(UART_BASE + 8)`，直接访问设备窗口之外，故与 PLIC 无关
  - probe 将 `UART_STRIDE` 误设为 4；`uart_16550 0.5.0` 使用 `base + logical_offset * stride`，QEMU virt 应为 stride 1
  - probe 只调用 `new_mmio()` 而未调用 `init(Config::default())`，即使删除越界写也不会开启 `IER.DATA_READY`
- **解决方案**: 将 stride 改为 1，删除所有越界 MMIO 诊断写，注册 handler 后执行 `uart.init(Config::default())`；handler 必须读取 IIR/LSR 并 drain RBR 清除 level-triggered RX 条件
- **预防**:
  - 区分平台 MMIO 页映射大小与设备寄存器窗口；页可写不代表页内任意地址都属于设备
  - 从设备树/平台契约确认 `reg-shift`，将 stride 作为硬件参数而不是驱动默认值
  - RED probe 必须先证明设备确实产生 IRQ，并确保 handler 能清除设备侧中断条件
- **修复执行（2026-06-19，M1-T1.3 RED Gate PASS）**:
  - `examples/uart_irq/src/main.rs` 修复 4 处缺陷并验证
  - UART_STRIDE `4 → 1`；删除 page table walk / PLIC 越界写 / UART+0x1000 / UART+8 / scause read
  - 注册顺序调整：`new_mmio → UART_VADDR.store → register → set_enable → init`（避免 stray IRQ 早返回）
  - handler 现在 `read_volatile(IIR) → loop read(LSR) & drain(RBR) → IRQ10_COUNT++ / RX_BYTE_COUNT+=drained`
  - QEMU 6s 跑出 503~519 个 heartbeat 全部 `count=0 rx=0`；`set_enable is not implemented for IRQ 10` warning 保留（RED 期望）
  - RED 日志：`/tmp/m1-t1-3-red.log`
  - GREEN 待 M1-T2.1 PLIC 移植完成后重跑
- **关联**: `examples/uart_irq/src/main.rs`, `.claude/analysis/m1-uart-irq-probe-blocker.md`, `openspec/changes/m1-riscv-plic-baseline/tasks.md` §1.3

## 技巧模式

### 1. axconfig-gen 配置生成

- **场景**: 需要调整 CPU 数量、内存布局、virtio 设备等
- **方法**: 修改 `configs/<arch>/*.toml`,运行 `axconfig-gen` 生成 `axconfig` crate 的 config.rs
- **注意**: `axconfig::plat::CPU_NUM` 已废弃(commit bcc354a),改用 `axplat::cpu::num()` 获取

### 2. QEMU 调试

- **场景**: 需要观察中断触发、寄存器、调度切换
- **方法**: `make A=examples/helloworld ARCH=riscv64 LOG=debug run`,默认 GDB 端口 1234
- **注意**: `-s` 与 `-S` 是 Makefile 已内置,无需手动加

### 3. RISC-V 编译运行快速上手

- **场景**: 使用 RISC-V 架构进行开发测试
- **环境要求**:
  - Rust nightly-2025-05-20 (自动由 rust-toolchain.toml 管理)
  - riscv64gc-unknown-none-elf 编译目标
  - qemu-system-riscv64 (版本 7.0.0+)
  - cargo-binutils, axconfig-gen, cargo-axplat
- **快速验证**:
  ```bash
  # 编译运行 helloworld
  make A=examples/helloworld ARCH=riscv64 run

  # 带日志级别
  make A=examples/helloworld ARCH=riscv64 LOG=info run

  # 多核测试
  make A=examples/helloworld ARCH=riscv64 SMP=4 run
  ```
- **输出示例**: OpenSBI 启动 → ArceOS Logo → 配置信息 → "Hello, world!"
- **注意**: 首次编译需下载依赖，约 6 秒完成

### 4. 环境配置检查清单

- **场景**: 新环境搭建或环境问题排查
- **检查项**:
  ```bash
  # 1. Rust 工具链
  rustup show  # 应显示 nightly-2025-05-20

  # 2. RISC-V 目标
  rustup target list --installed | grep riscv64

  # 3. QEMU
  qemu-system-riscv64 --version

  # 4. cargo 工具
  cargo install --list | grep -E "(cargo-binutils|axconfig-gen|cargo-axplat)"

  # 5. dosfstools (生成 disk.img 必需)
  which mkfs.fat
  ```
- **常见问题**:
  - 工具链不匹配 → 检查 rust-toolchain.toml
  - QEMU 未安装 → `sudo apt-get install qemu-system`
  - cargo 工具缺失 → `cargo install cargo-binutils axconfig-gen cargo-axplat`
  - **disk_img 报 `mkfs.fat: No such file or directory` → `sudo apt install dosfstools`**

### 5. arceos shell 启动完整配方 (riscv64)

- **场景**: 拿到一个能跑通 UART 收发的最小环境,作为异步 UART 改造的验证基线
- **先决条件**:
  - `mkfs.fat` 已装
  - 已执行 `make A=examples/shell ARCH=riscv64 disk_img` 生成 `disk.img`
- **命令**:
  ```bash
  make A=examples/shell ARCH=riscv64 BLK=y APP_FEATURES=use-ramfs run
  ```
- **预期输出**:
  ```
  Initialize platform devices...
  Initialize filesystems...
    use block device 0: "virtio-blk"
  Initialize filesystems done
  ...
  arceos:/$
  ```
- **可用命令**: `cat / cd / echo / exit / help / ls / mkdir / pwd / rm / uname`
- **挂载点**: `/dev` (devfs) + `/proc` (procfs) + `/sys` (sysfs) + `/tmp` (ramfs)
- **验证串口透传**:
  ```bash
  echo hello-async-uart > /tmp/test
  cat /tmp/test    # 应回显 hello-async-uart
  ls /tmp          # 应看到 test
  ```
- **退出 QEMU**: `Ctrl+A` 然后 `X`
- **关联**: P01 / P02 / P03 三条踩坑档案

## 文件速查表

| 路径 | 用途 |
|------|------|
| `Cargo.toml` | workspace 入口,所有 members 注册处 |
| `rust-toolchain.toml` | 工具链与目标 triple 锁定 |
| `Makefile` | 构建/运行入口,支持 ARCH/SMP/LOG/NET 等参数 |
| `configs/` | 各架构 TOML 配置,axconfig-gen 输入 |
| `modules/axhal/src/arch/<arch>/` | 各架构特定实现 |
| `modules/axhal/src/platform/` | 各 platform(QEMU virt/pc-q35,raspi4 等)实现 |
| `modules/axdriver/src/` | 驱动实现,每个外设独立子模块 |
| `modules/axruntime/src/lib.rs` | 内核初始化顺序与子系统装载入口 |
| `api/arceos_api/src/` | 用户态 syscall 暴露 |
| `ulib/axstd/src/` | no_std std 替代库 |
| `doc/` | 设计与架构文档 |
| `modules/axfs/src/lib.rs:39-45` | `init_filesystems`,块设备必填的 panic 点 |
| `modules/axfs/src/root.rs:149-178` | `init_rootfs`,myfs/fatfs 分支 + devfs/procfs/sysfs/ramfs 挂载 |
| `examples/shell/Cargo.toml:10` | `use-ramfs` feature 定义位置(app 层) |
| `examples/shell/src/ramfs.rs:8-15` | `MyFileSystemIf` 的 RamFileSystem 实现 |
| `examples/shell/src/cmd.rs:23-28` | shell 支持的命令注册表 |
| `scripts/make/qemu.mk:54-58` | `BLK=y` 时 QEMU 加 virtio-blk + disk 镜像 |
| `scripts/make/utils.mk:15-22` | `make_disk_image` 定义(mkfs.fat 依赖) |

## 串口与异步 I/O 探究反哺

| 编号 | 名称 | 路径 | 用途 | 时间 |
|------|------|------|------|------|
<!-- L01 --> | ArceOS console 门面 | `modules/axhal/src/lib.rs` → `axplat::console::ConsoleIf` | 平台编译期选择 console 后端；RISC-V 走 SBI，x86 走同步 16550 | 2026-06-19 |
<!-- L02 --> | ArceOS IRQ 门面 | `modules/axhal/src/irq.rs` → `axplat::irq::IrqIf` | trap 中转、handler 注册和设备 IRQ 分派 | 2026-06-19 |
<!-- L03 --> | uart_16550 异步核心 | `../uart_16550/src/async_/` | ISR、SPSC ring、RX/TX copier 和 async device ops | 2026-06-19 |
<!-- L04 --> | uart_16550 OS 抽象 | `../uart_16550/src/os/mod.rs` | `OsRuntime`/`OsIrq`/`OsMmio`/`OsSpinNoIrq`/`OsWakerSet` 移植接口 | 2026-06-19 |

<!-- L05 --> ### RISC-V QEMU virt 的 console 不是 MMIO UART

- `axplat-riscv64-qemu-virt 0.4.1` 的 `ConsoleIf` 使用 SBI DBCN 收发。
- 平台虽配置 UART `0x1000_0000` 和 PLIC `0x0c00_0000` MMIO 范围，但 PLIC enable/claim/complete 尚未实现。
- 在该平台验证异步 UART 前，必须先补 PLIC；early console 应继续保留 SBI。

<!-- L06 --> ### axtask 当前不提供 Future executor

- `axtask::spawn`/`spawn_raw` 接受 `FnOnce()`，不能直接实现 `OsRuntime::spawn(Future)`。
- 网络模块的 `block_on` 是 `WouldBlock + yield_now` 重试，不是 Future executor。
- UART copier 集成需要最小单 Future executor 或通用内核 executor，不能用 yield 轮询替代。

<!-- L07 --> ### StarryOS 异步 UART 的实际 ArceOS 基线

- 已验证实现依赖 `axtask/axhal 0.3.0-preview.2`、`axplat-riscv64-qemu-virt 0.3.1-pre.6` 和 `axpoll 0.1`，不是当前 ArceOS 0.2 API 基线。
- `axtask 0.3` 提供 `future::block_on`，通过 task waker、`blocked_resched` 和 `unblock_task` 实现无轮询等待。
- 0.3.1-pre.6 的 RISC-V platform 已通过 `riscv_plic` 实现 priority、enable、claim 和 complete；当前 0.4.1 反而仍是 TODO。

<!-- L08 --> ### StarryOS 已验证的 UART 初始化顺序

`MMIO 验证 -> static ring 初始化 -> driver Once/Arc -> ISR wrapper 注册 -> RX/TX copier spawn -> TTY/readiness 绑定`。必须在开启 UART/PLIC 中断前完成 driver 和 waker 初始化，避免早到 IRQ。

<!-- L09 --> ### ArceOS 0.2 无需回移全局 IRQ hook

StarryOS 使用 `register_irq_hook(fn(usize))` 是 0.3 架构选择。当前 ArceOS 已有 `axhal::irq::register(irq, fn())`，单 UART adapter 可用静态无参 trampoline 绑定 UART IRQ 10、MMIO base 和 cached IER。

<!-- L10 --> ### parity-first 迁移边界

首个 ArceOS PoC 固定单 UART、单 reader、单 writer，并保留 SBI console。先复现 StarryOS 的 ISR/copy/ring 闭环，再处理多生产者、per-port waker、真正 async flush 和 TTY/POSIX 集成。

<!-- L11 --> ### UART IRQ probe 的因果有效性条件

UART RED/GREEN probe 必须同时满足：平台 stride/base/IRQ 正确；设备 IER 已开启；handler 可识别并清除 IIR/LSR/RBR 对应的 level condition。仅注册 PLIC handler 或仅观察计数，都不足以证明 PLIC 路径正确。

**2026-06-19 验证（M1-T1.3 RED Gate PASS）**: 四条件均已在 `examples/uart_irq` 中满足并通过 QEMU 见证：
1. **stride/base/IRQ 正确** — `UART_STRIDE=1` 匹配 ns16550a `reg-shift=0`；base `0x1000_0000` 来自 axconfig；IRQ 10 = QEMU virt PLIC source
2. **IER 已开启** — `uart.init(Config::default())` 启用 `IER.DATA_READY`
3. **handler 清源** — handler `read(IIR)` + `loop { read(LSR) & drain(RBR) }` 清除 level condition
4. **双计数** — `IRQ10_COUNT`（分派次数）+ `RX_BYTE_COUNT`（消费字节）独立追踪，可区分「控制器分派」与「设备消费」

RED 日志 `/tmp/m1-t1-3-red.log` 显示 6s 内 503~519 个 heartbeat 全部 `count=0 rx=0`，且保留 `set_enable is not implemented for IRQ 10` warning —— 此 warning 是 PLIC TODO 的预期信号，不是 probe 失败。GREEN 待 M1-T2.1 完成 PLIC 移植后重跑（应看到 host byte → `count++ & rx++` 且无 IRQ storm）。

<!-- L12 --> ### PLIC 移植：MMIO 映射时序必须在 `init_later`

`init_early` 在 axhal 内存管理之前执行，此时 PLIC VADDR（`0xffffffc00c000000`）尚未建立页表映射。尝试在 `init_early` 中调用 `phys_to_virt(PLIC_PADDR)` 虽然只是算术运算不会崩溃，但后续 `init_by_context()` 的 MMIO 写入会触发 StoreFault。必须将 `plic::init()` 和 `plic::init_percpu()` 移到 `init_later`（页表已就绪）。

**2026-06-19 验证**: 初始实现在 `init_early` 中调用 `plic::init()` 导致 `IllegalInstruction` trap。移至 `init_later` 后 PLIC 初始化日志正常（`PLIC init base_vaddr=0xffffffc00c000000`）。

<!-- L13 --> ### RISC-V `mhartid` CSR 在 S-mode 不可读

`riscv::register::mhartid::read()` 在 QEMU virt S-mode 下触发 `IllegalInstruction` trap。`mhartid` 是 M-mode CSR，默认不委托给 S-mode。解决方案：使用 `init_percpu(cpu_id)` 的 `cpu_id` 参数替代 CSR 读取；对于 IRQ handler 中需要当前 context 的场景，在 `init_percpu` 时存入 `CURRENT_CONTEXT: AtomicUsize` 静态变量。

**2026-06-19 验证**: `mhartid::read()` 在 S-mode 触发 panic；改用 `cpu_id` 参数传递后正常。

<!-- L14 --> ### 16550 UART IRQ 需要 MCR.OUT2 + FCR trigger ≤ RX bytes

`uart_16550::Config::default()` 正确设置了 `IER::DATA_READY`，但有两个隐藏条件：
1. **MCR.OUT2**（bit 3）：16550 中断输出线必须通过 MCR.OUT2 使能。虽然 `Uart16550::init()` 已设置 `MCR::OUT_2_INT_ENABLE`（0.5.0），但禁用 FIFO 后可能需要手动确认。
2. **FCR 触发级别**：`Config::DEFAULT` 使用 `FifoTriggerLevel::Fourteen`，RX FIFO 必须积满 14 字节才触发 IRQ。测试时只发 1-2 字节则 IRQ 永远不会触发。

**2026-06-19 验证**: 初始 probe 用 `Config::default()` 时 `PLIC pending[10]=0`（IRQ 未触发），改 FCR=0x01（trigger=1 byte）后 `claim_reg=0x0A`（IRQ 触发成功）。

<!-- L15 --> ### PLIC S_EXT trap 不触发：先检查顶层 feature 是否开启全局 SIE

S_EXT 中断不触发时的排查顺序：
1. 读 UART LSR → 确认数据到达（排除输入路径问题）
2. 读 PLIC claim 寄存器（`base + 0x200000 + ctx*0x1000 + 4`）→ 确认 PLIC 已注册 IRQ
3. 读 UART IIR → 确认设备级中断状态
4. 读 UART IER/MCR/FCR → 确认设备中断配置
5. 读 PLIC priority/enable/threshold → 确认 PLIC 配置
6. 读 `sie`/`sstatus` CSR → 必须同时满足 `sie.SEIE=1` 与 `sstatus.SIE=1`
7. 检查应用 feature graph → `axhal/irq` 只提供 HAL 能力，不会自动启用 `axruntime/irq`
8. 仅当以上均正确但 trap 不触发时，再排查 PLIC target/QEMU wiring

**2026-06-19 根因修正**：OpenSBI 日志 `MIDELEG=0x1666` 已包含 SEIP bit 9，委托正常。`examples/uart_irq` 只启用 `axhal/irq`，未启用 `axstd/irq -> axfeat/irq -> axruntime/irq`；因此 `axruntime::init_interrupt()` 未编译，启动日志缺少 `Initialize interrupt handlers...`，最终 `axhal::asm::enable_irqs()` 未执行、`sstatus.SIE=0`。手工读取 claim register 得到 10 只能证明设备到 PLIC context 的路径，并且该读取会消费 claim。

<!-- L16 --> ### ArceOS IRQ feature 必须从顶层能力入口传播

- 应用需要中断时优先启用 `axstd/irq`（或等价的 `axfeat/irq`），其传播链为 `axstd/irq -> axfeat/irq -> axhal/irq + axruntime/irq + axtask?/irq`。
- 仅在直接依赖项上启用 `axhal/irq`，只会编译 HAL IRQ API 和平台实现；不会启用 runtime 的 timer handler 注册及全局 `enable_irqs()`。
- 诊断方法：`cargo tree -e features -i axruntime` 必须出现 `axruntime feature "irq"`，运行日志必须出现 `Initialize interrupt handlers...`。
- PLIC claim register 是有副作用的 claim 操作，主循环诊断不得轮询它；使用 pending bitmap 和 CSR 做无侵入观察。

**2026-06-19 M1 GREEN Gate 最终验证**:
- `examples/uart_irq/Cargo.toml` 添加 `axstd/irq` 后，启动日志出现 `Initialize interrupt handlers...`
- 注入 "HELLO" (5B) → IRQ10 count=5 rx=5；注入 "0123456789" (10B) → count=10；注入 64B → count=64
- T4.1 SBI console 回归：`examples/helloworld` 正常输出 "Hello, world!"
- T4.2 1B/64B loop：10×10B→count=10 一致；64B→count=64 一致
- T4.3 idle 12s：count=0，无 spurious loop，无 panic
- T4.4 build matrix：x86_64/aarch64/loongarch64 全部编译通过
- M1 commit: `081680f` (14/15 tasks)

<!-- L17 --> ### QEMU TCP serial 自动化测试的时序 trap

QEMU `-serial tcp:...` 配合 `nc` 做自动化字节注入时，有三个致命问题：
1. **tick 速率不稳定**：CPU loop counter（`tick.wrapping_add(1)`）速率 ~100M ticks/s，且每次 QEMU 启动略有不同，不能用于精确时钟阈值
2. **字节到达延迟**：`nc -N` 发送字节到 TCP socket 后，可能要数秒才被 QEMU 转发到 guest UART，期间 guest 已经跑过数十亿 tick
3. **端口复用**：多次重启 QEMU 时，tcp server 需要 `server=on,wait=off` 否则端口被占用

正确做法：**不要用 tick 阈值做自动化测试的时间基准**。要么用真实的 RISC-V mtime CSR（10MHz 定时器），要么用外部 expect/pexpect 脚本等待特定输出字符串后再注入字节。

<!-- L18 --> ### block_on 丢失唤醒竞态窗口（已修复）

**症状**：Future 返回 Pending 后任务永久阻塞，即使 Waker 已被调用。在 `test_future_block_on_wake_from_peer` 等竞态测试中表现为挂起（不完成）。

**根因**：`block_on` 的唤醒握手存在竞态窗口：

```
// ❌ 错误顺序
drop(woke_guard);       // 释放锁 → 竞态窗口打开
rq.block_current();     // set_state(Blocked) 在锁释放之后
```

竞态链条：
1. Waker 获取 `woke` 锁 → 设 `woke = true` → 调用 `unblock_task`
2. 但 `unblock_task` 依赖任务已处于 `Blocked` 状态才能转换 `Blocked → Ready`
3. 此时任务仍是 `Running` → `unblock_task` 为 NO-OP
4. `block_current` 随后设 `Blocked` → 任务永久阻塞
5. 下一轮循环开头 `*woke.lock() = false` 覆盖唤醒标志 → 证据丢失

**解决**：匹配 `blocked_resched` 的现有模式 — **持锁改状态，放锁后切换**：

```rust
// ✅ 正确顺序
curr.set_state(TaskState::Blocked);  // 持锁改状态（woke_guard 仍在）
drop(woke_guard);                     // 放锁
self.inner.resched();                 // 切换 -> 上下文切换
```

关键约束：`woke_guard` 必须跨越 `set_state(Blocked)` 以阻止 Waker 的 `unblock_task` 在 NO-OP 窗口执行；但必须在 `resched()` 前释放以避免 Waker 在等待锁时死锁。

**修改文件**：`modules/axtask/src/run_queue.rs` (`block_current` 签名改为 `fn block_current(&mut self, woke_guard: SpinNoIrqGuard<'_, bool>)`)

**预防**：任何新增的阻塞原语如果不由 WaitQueue 保护，必须：1) 在状态迁移期间持有 Waker 可观察的同步原语；2) 在调用 `resched()` 前显式释放该原语以避免跨上下文切换持有锁。

<!-- L19 --> ### axtask RR 调度器测试在用户态全量失败的根因

**症状**：
- `cargo test -p axtask --features "sched-rr"` 4 个既有测试全部失败
- `test_wait_queue` / `test_task_join` → `assertion failed: curr.can_preempt(2)`
- `test_sched_fifo` / `test_fp_state_switch` → `SIGABRT: failed to initiate panic, error 5`

**根因**：**不是 RR 调度器本身有 bug**，而是用户态测试基础设施不支持 `preempt`。

依赖链：`sched-rr → preempt → irq → kernel_guard/preempt → percpu/preempt`

`blocked_resched` 中 `can_preempt(2)` 断言的期望是：
- 1 层来自 `NoPreemptIrqSave`（run queue guard）
- 1 层来自 `SpinNoIrq`（wait queue lock）

用户态测试使用 `percpu/sp-naive`（单处理器 naive 实现），该实现在跨 `resched()` 上下文切换后无法正确追踪抢占计数的累加/递减，导致计数不匹配。`SIGABRT` 的 root cause 相同——panic handler 的 console 输出路径也可能经过 `blocked_resched` 或在持锁状态下触发二次 panic。

**为什么 `block_on` 不受影响**：M2 的 `block_current` 不走 `blocked_resched`，不经过 WaitQueue 路径，不触发该断言。6 个 future 测试在 RR 下全部通过。

**影响范围**：仅限 `cargo test` 用户态环境；QEMU/bare-metal 的 RR 调度不受影响。

**诊断日期**：2026-06-19（M2 验证期间发现）
