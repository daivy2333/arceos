#![no_std]
#![no_main]

#[macro_use]
extern crate axstd as std;

use core::sync::atomic::{AtomicUsize, Ordering};
use core::ptr;

use memory_addr::{pa, va};
use uart_16550::Config;
#[cfg(target_arch = "riscv64")]
use uart_16550::Uart16550;

/// QEMU virt machine constants (not yet exposed by axconfig::devices).
#[cfg(target_arch = "riscv64")]
mod plat {
    /// 16550 UART0 MMIO base on QEMU virt.
    pub const UART_PADDR: usize = 0x1000_0000;
    /// Stride between adjacent UART registers.
    pub const UART_STRIDE: u8 = 4;
    /// PLIC source number for UART0 RX on QEMU virt.
    pub const UART_IRQ: usize = 10;
}

/// Atomic counter for IRQ 10 firings. Updated from the handler, read from main.
static IRQ10_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Trap handler. axplat 0.4 `IrqHandler = fn()` (no argument), so the handler
/// must match that signature.
fn irq10_handler() {
    IRQ10_COUNT.fetch_add(1, Ordering::Relaxed);
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

    // Direct MMIO write test: write 'A' to UART THR (offset 0).
    let thr_addr = uart_vaddr as *mut u8;
    unsafe {
        ptr::write_volatile(thr_addr, b'A');
    }
    sbi_puts("[uart_irq] direct write 'A' to THR ok\n");

    // SAFETY: uart_vaddr points at the kernel-mapped 16550 MMIO region.
    let mut uart = unsafe {
        Uart16550::new_mmio(thr_addr, plat::UART_STRIDE)
            .expect("UART MMIO init must succeed on QEMU virt")
    };
    sbi_puts("[uart_irq] new_mmio ok\n");

    // Dump the actual page table root from satp, then read the L2 entry that
    // maps our VA range. This is the real source of truth — the boot PT
    // described in axplat boot.rs is no longer the one in use after
    // axmm::init_memory_management() runs.
    unsafe {
        let satp: usize;
        core::arch::asm!("csrr {0}, satp", out(reg) satp);
        sbi_puts("[uart_irq] satp=");
        sbi_puts(hex_str(satp));
        sbi_puts("\n");
        // RV64 satp layout: MODE[63:60] | ASID[59:44] | PPN[43:0]
        let mode = (satp >> 60) & 0xf;
        let ppn = satp & ((1usize << 44) - 1);
        let root_pa = ppn << 12;
        sbi_puts("[uart_irq] mode=");
        sbi_puts(usize_str(mode));
        sbi_puts(" root_pa=");
        sbi_puts(hex_str(root_pa));
        sbi_puts("\n");

        // Walk: VA 0xffffffc010000000
        //   VPN[2] = (VA >> 30) & 0x1ff = 0x100
        //   VPN[1] = (VA >> 21) & 0x1ff = (0x1000_0000 >> 21) & 0x1ff = 0x00
        //   VPN[0] = (VA >> 12) & 0x1ff = (0x1000_0000 >> 12) & 0x1ff = 0x00
        let root_va = axhal::mem::phys_to_virt(pa!(root_pa)).as_usize() as *const u64;
        let l2_pte = ptr::read_volatile(root_va.add(0x100));
        sbi_puts("[uart_irq] L2[0x100]=");
        sbi_puts(hex_str(l2_pte as usize));
        sbi_puts(" V=");
        sbi_puts(if l2_pte & 1 != 0 { "1" } else { "0" });
        sbi_puts(" R=");
        sbi_puts(if l2_pte & 2 != 0 { "1" } else { "0" });
        sbi_puts(" W=");
        sbi_puts(if l2_pte & 4 != 0 { "1" } else { "0" });
        sbi_puts(" X=");
        sbi_puts(if l2_pte & 8 != 0 { "1" } else { "0" });
        sbi_puts("\n");

        // If L2 is non-leaf (V=1 but R=X=0), walk down to L1.
        let l2_is_leaf = (l2_pte & 0xf) == 0x1 || (l2_pte & 0xa) != 0;
        sbi_puts("[uart_irq] L2 is_leaf=");
        sbi_puts(if l2_is_leaf { "yes" } else { "no" });
        sbi_puts("\n");

        // Walk L1 and L0 for the UART range regardless, to see if any
        // override exists (the page table may not implement non-leaf override
        // but checking is cheap).
        // L2 PPN bits: PPN[2] = bits 53-28 of PTE (for 1G block at level 2).
        // L2 points to L1 if non-leaf. Treat it as non-leaf pointer either way.
        let l1_pa = ((l2_pte as usize >> 10) & 0x0fff_ffff_ffff) << 12;
        sbi_puts("[uart_irq] L1 phys=");
        sbi_puts(hex_str(l1_pa));
        sbi_puts("\n");
        if l1_pa != 0 {
            let l1_va = axhal::mem::phys_to_virt(pa!(l1_pa)).as_usize() as *const u64;
            for i in 0..4 {
                let pte = ptr::read_volatile(l1_va.add(i));
                if pte & 1 != 0 {
                    sbi_puts("[uart_irq] L1[");
                    sbi_puts(usize_str(i));
                    sbi_puts("]=");
                    sbi_puts(hex_str(pte as usize));
                    sbi_puts(" V=");
                    sbi_puts(if pte & 1 != 0 { "1" } else { "0" });
                    sbi_puts(" R=");
                    sbi_puts(if pte & 2 != 0 { "1" } else { "0" });
                    sbi_puts(" W=");
                    sbi_puts(if pte & 4 != 0 { "1" } else { "0" });
                    sbi_puts(" X=");
                    sbi_puts(if pte & 8 != 0 { "1" } else { "0" });
                    sbi_puts("\n");
                }
            }
        }
    }
    sbi_puts("[uart_irq] page table dump done\n");

    sbi_puts("[uart_irq] page table dump done\n");

    // Test write to PLIC MMIO (0xc00_0000) and try writing past PLIC's range.
    let plic_paddr: usize = 0x0c00_0000;
    let plic_vaddr = phys_to_virt(pa!(plic_paddr)).as_usize();
    sbi_puts("[uart_irq] plic_vaddr=");
    sbi_puts(hex_str(plic_vaddr));
    sbi_puts("\n");
    unsafe {
        ptr::write_volatile(plic_vaddr as *mut u32, 0xdeadbeef);
        sbi_puts("[uart_irq] write plic off=0 ok\n");
        // PLIC size is 0x21_0000. Try past that.
        ptr::write_volatile((plic_vaddr + 0x21_0000) as *mut u32, 0xdeadbeef);
        sbi_puts("[uart_irq] write plic off=0x210000 ok\n");
    }

    // Test: try writing to UART + 0x1000 (4K page boundary)
    unsafe {
        ptr::write_volatile(thr_addr.add(0x1000), 0x01u8);
        sbi_puts("[uart_irq] write uart off=0x1000 ok\n");
    }

    // Read scause and stval right before the failing write to diagnose.
    let scause: usize;
    let stval: usize;
    let sscratch: usize;
    unsafe {
        core::arch::asm!(
            "csrr {0}, scause",
            "csrr {1}, stval",
            "csrr {2}, sscratch",
            out(reg) scause,
            out(reg) stval,
            out(reg) sscratch,
        );
    }
    sbi_puts("[uart_irq] pre-write scause=");
    sbi_puts(usize_str(scause));
    sbi_puts(" stval=");
    sbi_puts(hex_str(stval));
    sbi_puts(" sscratch=");
    sbi_puts(hex_str(sscratch));
    sbi_puts("\n");

    // Test: try writing to UART offset 8.
    unsafe {
        let addr8 = thr_addr.add(8);
        ptr::write_volatile(addr8, 0x01u8);
        sbi_puts("[uart_irq] write off=8 (precomp ptr) ok\n");
    }

    // Register IRQ 10 handler via axhal facade.
    let ok = register(plat::UART_IRQ, irq10_handler);
    sbi_puts("[uart_irq] register(10)=");
    sbi_puts(if ok { "true" } else { "false" });
    sbi_puts("\n");
    set_enable(plat::UART_IRQ, true);
    sbi_puts("[uart_irq] set_enable(10,true) called\n");

    // Main loop: read UART, echo bytes, periodically report count via SBI.
    let mut last_reported: usize = 0;
    let mut buf = [0u8; 16];
    let mut tick: u32 = 0;
    loop {
        let n = uart.try_receive_bytes(&mut buf);
        if n > 0 {
            for &b in &buf[..n] {
                let _ = uart.try_send_byte(b);
            }
        }
        let cur = IRQ10_COUNT.load(Ordering::Relaxed);
        if cur != last_reported {
            sbi_puts("[uart_irq] IRQ10 count=");
            sbi_puts(usize_str(cur));
            sbi_puts("\n");
            last_reported = cur;
        }
        tick = tick.wrapping_add(1);
        if tick % 5_000_000 == 0 {
            sbi_puts("[uart_irq] heartbeat tick=");
            sbi_puts(u32_str(tick));
            sbi_puts(" count=");
            sbi_puts(usize_str(cur));
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
