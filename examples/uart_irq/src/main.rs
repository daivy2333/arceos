#![no_std]
#![no_main]

extern crate axstd as std;

use core::sync::atomic::{AtomicUsize, Ordering};
use core::ptr;

use memory_addr::{pa, va};
use uart_16550::Config;
#[cfg(target_arch = "riscv64")]
use uart_16550::Uart16550;

/// QEMU virt machine constants (per hardware contract, not driver defaults).
#[cfg(target_arch = "riscv64")]
mod plat {
    /// 16550 UART0 MMIO base on QEMU virt.
    pub const UART_PADDR: usize = 0x1000_0000;
    /// Stride between adjacent UART registers. QEMU virt's device tree exposes
    /// the UART as `ns16550a` with no `reg-shift` property, so logical register
    /// offsets map to consecutive byte addresses. Using `4` would make logical
    /// FCR (offset 2) land at physical offset 8, which is past the 8-byte
    /// device window and triggers StoreFault on first `init()` write.
    pub const UART_STRIDE: u8 = 1;
    /// PLIC source number for UART0 RX on QEMU virt.
    pub const UART_IRQ: usize = 10;
}

/// Handler-entry counter (PLIC dispatch count).
static IRQ10_COUNT: AtomicUsize = AtomicUsize::new(0);
/// Bytes drained from RBR by the handler (level-triggered clear count).
/// 16550 RX IRQ is level-triggered on LSR.DATA_READY; without draining RBR
/// in the handler, PLIC would re-fire after complete and cause IRQ storm.
static RX_BYTE_COUNT: AtomicUsize = AtomicUsize::new(0);
/// Kernel VA of UART MMIO base. Set by `run_riscv` before IRQ register,
/// read by the handler to drain RBR / inspect IIR / inspect LSR.
static UART_VADDR: AtomicUsize = AtomicUsize::new(0);

/// Trap handler. `axplat 0.4 IrqHandler = fn()` (no argument), so the handler
/// must match that signature. Drains RBR to clear the UART's level-triggered
/// RX condition so PLIC won't re-fire after complete.
fn irq10_handler() {
    IRQ10_COUNT.fetch_add(1, Ordering::Relaxed);

    let base = UART_VADDR.load(Ordering::Relaxed);
    if base == 0 {
        return;
    }

    // SAFETY: `base` is the kernel VA of the UART MMIO region mapped W=R.
    // 16550 register offsets: IIR=2 (RO), LSR=5 (RO), RBR=0 (RO).
    unsafe {
        // Identify interrupt reason via IIR (read-only; clears the IIR read).
        let _iir = ptr::read_volatile((base + 2) as *const u8);
        // Drain all available bytes while LSR.DATA_READY (bit 0) is set.
        let mut drained = 0usize;
        loop {
            let lsr = ptr::read_volatile((base + 5) as *const u8);
            if lsr & 0x01 == 0 {
                break;
            }
            let _byte = ptr::read_volatile(base as *const u8);
            drained += 1;
        }
        RX_BYTE_COUNT.fetch_add(drained, Ordering::Relaxed);
    }
}

/// Write a string to SBI console (byte-by-byte, no buffering).
/// Marked `#[inline(never)]` to keep the BUF static at its real .bss address
/// instead of being inlined and register-allocated as the UART vaddr.
#[cfg(target_arch = "riscv64")]
#[inline(never)]
fn sbi_puts(s: &str) {
    for &b in s.as_bytes() {
        unsafe {
            // BUF is a single-byte scratch in .bss. The compiler may otherwise
            // allocate it to a register that happens to hold the UART vaddr.
            static mut BUF: [u8; 1] = [0];
            BUF[0] = b;
            let vaddr = core::ptr::addr_of_mut!(BUF) as usize;
            let paddr = axhal::mem::virt_to_phys(va!(vaddr)).as_usize();
            sbi_rt::console_write(sbi_rt::Physical::new(1, paddr, 0));
        }
    }
}

#[unsafe(no_mangle)]
fn main() {
    sbi_puts("[uart_irq] probe start\n");

    #[cfg(target_arch = "riscv64")]
    run_riscv();

    #[cfg(not(target_arch = "riscv64"))]
    sbi_puts("[uart_irq] only riscv64\n");
}

#[cfg(target_arch = "riscv64")]
fn run_riscv() {
    use axhal::irq::{register, set_enable};
    use axhal::mem::phys_to_virt;

    // Translate the physical MMIO address to the kernel virtual address.
    let uart_vaddr = phys_to_virt(pa!(plat::UART_PADDR)).as_usize();
    sbi_puts("[uart_irq] uart_vaddr=");
    sbi_puts(hex_str(uart_vaddr));
    sbi_puts("\n");

    // SAFETY: uart_vaddr points at the kernel-mapped 16550 MMIO region.
    let mut uart = unsafe {
        Uart16550::new_mmio(uart_vaddr as *mut u8, plat::UART_STRIDE)
            .expect("UART MMIO init must succeed on QEMU virt")
    };
    sbi_puts("[uart_irq] new_mmio ok\n");

    // Publish the kernel VA of the UART MMIO region for the handler BEFORE
    // registering IRQ 10, so the handler can read IIR / LSR / RBR without
    // borrowing `uart` and never early-returns if a stray IRQ fires after
    // register but before init.
    UART_VADDR.store(uart_vaddr, Ordering::Relaxed);

    // Register IRQ 10 handler via axhal facade.
    // RED expected: `register` succeeds but `set_enable` only logs a warning
    // because vendor axplat-riscv64-qemu-virt 0.4.1 has PLIC TODO. The handler
    // therefore never fires; irq_count/rx_byte_count must stay at 0 until
    // PLIC is implemented (M1 T2.1).
    let ok = register(plat::UART_IRQ, irq10_handler);
    sbi_puts("[uart_irq] register(10)=");
    sbi_puts(if ok { "true" } else { "false" });
    sbi_puts("\n");
    set_enable(plat::UART_IRQ, true);
    sbi_puts("[uart_irq] set_enable(10,true) called\n");

    // Initialize UART to enable IER.DATA_READY so the device actually
    // produces level-triggered RX IRQ 10 on host byte input. Without this,
    // the device never raises IRQ 10 and the probe cannot validate PLIC
    // even after it is implemented.
    uart.init(Config::default()).expect("UART init must succeed on QEMU virt");
    sbi_puts("[uart_irq] init ok (IER.DATA_READY enabled)\n");

    // Main loop: periodically report both counters via SBI console. Handler
    // drains RBR, so we no longer poll the device here; that avoids racing
    // with the handler over RX bytes during GREEN.
    let mut last_irq: usize = 0;
    let mut last_rx: usize = 0;
    let mut tick: u32 = 0;
    loop {
        let cur_irq = IRQ10_COUNT.load(Ordering::Relaxed);
        let cur_rx = RX_BYTE_COUNT.load(Ordering::Relaxed);
        if cur_irq != last_irq || cur_rx != last_rx {
            sbi_puts("[uart_irq] IRQ10 count=");
            sbi_puts(usize_str(cur_irq));
            sbi_puts(" rx_byte_count=");
            sbi_puts(usize_str(cur_rx));
            sbi_puts("\n");
            last_irq = cur_irq;
            last_rx = cur_rx;
        }
        tick = tick.wrapping_add(1);
        if tick % 5_000_000 == 0 {
            sbi_puts("[uart_irq] heartbeat tick=");
            sbi_puts(u32_str(tick));
            sbi_puts(" count=");
            sbi_puts(usize_str(cur_irq));
            sbi_puts(" rx=");
            sbi_puts(usize_str(cur_rx));
            sbi_puts("\n");
        }
    }
}

/// Format a usize as decimal (no std, no alloc).
fn usize_str(mut n: usize) -> &'static str {
    // Tiny ring buffer
    const CAP: usize = 20;
    static mut BUF: [u8; CAP] = [0; CAP];
    unsafe {
        if n == 0 {
            BUF[0] = b'0';
            return core::str::from_utf8_unchecked(&BUF[..1]);
        }
        let mut i = CAP;
        while n > 0 && i > 0 {
            i -= 1;
            BUF[i] = b'0' + (n % 10) as u8;
            n /= 10;
        }
        core::str::from_utf8_unchecked(&BUF[i..])
    }
}

fn u32_str(mut n: u32) -> &'static str {
    const CAP: usize = 20;
    static mut BUF: [u8; CAP] = [0; CAP];
    unsafe {
        if n == 0 {
            BUF[0] = b'0';
            return core::str::from_utf8_unchecked(&BUF[..1]);
        }
        let mut i = CAP;
        while n > 0 && i > 0 {
            i -= 1;
            BUF[i] = b'0' + (n % 10) as u8;
            n /= 10;
        }
        core::str::from_utf8_unchecked(&BUF[i..])
    }
}

fn hex_str(mut n: usize) -> &'static str {
    const CAP: usize = 18;
    static mut BUF: [u8; CAP] = [0; CAP];
    unsafe {
        if n == 0 {
            BUF[0] = b'0';
            return core::str::from_utf8_unchecked(&BUF[..1]);
        }
        let mut i = CAP;
        while n > 0 && i > 0 {
            i -= 1;
            let d = (n & 0xf) as u8;
            BUF[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
            n >>= 4;
        }
        // 0x prefix
        if i >= 2 {
            i -= 2;
            BUF[i] = b'x';
            BUF[i + 1] = b'0';
        }
        core::str::from_utf8_unchecked(&BUF[i..])
    }
}