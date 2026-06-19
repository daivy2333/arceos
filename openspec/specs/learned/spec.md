## Purpose

记录 arceos 开发过程中的学习记忆(API 路径、踩坑档案、技巧模式、文件速查),避免重复探索,加速问题定位与决策。

> Version: 0.2.0  
> Last updated: 2026-06-19  
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

### P04: QEMU RISC-V 16550 设备 MMIO 范围只 8 字节,4K 页内 offset ≥8 写触发 StoreFault

- **症状**: 在 `examples/uart_irq` probe 中,直接 MMIO 写 UART 偏移 8 触发 kernel panic
  ```
  [uart_irq] write plic off=0x210000 ok       # 任意 PLIC 偏移都 OK
  [uart_irq] write uart off=0x1000 ok         # 4K 边界 OK
  [panic] Unhandled trap Exception(StoreFault) @ sb s1, 8(s2)
  ```
  注:偏移 0..7 可写并出字符,偏移 8 触发 CPU StoreFault,不是 QEMU "无设备" 静默忽略。
- **根因(待定)**:
  - 1G page table entry 正确(`L2[0x100]=0xef V=R=W=X=1`)
  - PLIC 在 0xc00_0000 范围任意 offset 都 OK
  - 唯一差异:UART 设备实体是 8 字节(16550 寄存器),QEMU RISC-V 可能把设备 size 之外配置成 fault 而不是 unassigned
  - QEMU virt 16550 (`hw/char/serial.c`):`addr >= ARRAY_SIZE(s->divider) && addr != 7` 静默 return
  - 怀疑 OpenSBI 配置的 PMP 或者 QEMU 内部 memory region 把 8..0xfff 当成 fault 区域
- **解决方案(临时)**: probe 不调用 `uart_16550::init()`,只手动写 IER (offset 1) + THR (offset 0) + LSR (offset 5) 这三个寄存器;FCR/SPR/MCR 等其他寄存器由 PLIC 实现完整后再补
- **预防**:
  - 直接 MMIO 16550 时,先只动 offset 0..7 的寄存器
  - 如需 FCR/SPR 等,用 `lcr_test_loopback` 通过 `init()` 一站式配,避免自己拼寄存器序列
  - 写之前先 `csrr scause/stval` 确认 CPU 状态
- **关联**: `examples/uart_irq/src/main.rs`, `vendor/axplat-riscv64-qemu-virt/src/boot.rs:11-21`, `QEMU hw/char/serial.c serial_mm_write`

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
