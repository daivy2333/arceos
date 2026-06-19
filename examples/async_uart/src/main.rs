#![no_std]
#![no_main]

extern crate alloc;
extern crate axstd as std;

mod adapter;

use core::future::poll_fn;
use core::sync::atomic::Ordering;
use core::task::Poll;

#[cfg(target_arch = "riscv64")]
fn sbi_puts(s: &str) {
    for &b in s.as_bytes() {
        unsafe {
            static mut BUF: [u8; 1] = [0];
            BUF[0] = b;
            let vaddr = core::ptr::addr_of_mut!(BUF) as usize;
            let paddr = axhal::mem::virt_to_phys(memory_addr::va!(vaddr)).as_usize();
            sbi_rt::console_write(sbi_rt::Physical::new(1, paddr, 0));
        }
    }
}

fn usize_str(mut n: usize) -> &'static str {
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

#[unsafe(no_mangle)]
fn main() {
    #[cfg(target_arch = "riscv64")]
    run_riscv();

    #[cfg(not(target_arch = "riscv64"))]
    axhal::console::write_bytes(b"[async_uart] riscv64 only\n");
}

#[cfg(target_arch = "riscv64")]
fn run_riscv() {
    sbi_puts("[async_uart] bootstrap start\n");

    let driver = adapter::init_uart();

    sbi_puts("[async_uart] bootstrap done, starting echo\n");

    axtask::block_on(echo_loop(driver));
}

#[cfg(target_arch = "riscv64")]
async fn echo_loop(driver: &'static adapter::Driver) {
    let mut buf = [0u8; 256];
    let mut total: usize = 0;

    loop {
        let n = poll_fn(|cx| {
            let n = driver.rx.pop(&mut buf);
            if n > 0 {
                return Poll::Ready(n);
            }
            driver.rx.register_waker(cx.waker());
            let n2 = driver.rx.pop(&mut buf);
            if n2 > 0 {
                Poll::Ready(n2)
            } else {
                Poll::Pending
            }
        })
        .await;

        let mut written = 0;
        while written < n {
            let w = poll_fn(|cx| {
                let w = driver.tx.push(&buf[written..n]);
                if w > 0 {
                    return Poll::Ready(w);
                }
                driver.tx.register_waker(cx.waker());
                let w2 = driver.tx.push(&buf[written..n]);
                if w2 > 0 {
                    Poll::Ready(w2)
                } else {
                    Poll::Pending
                }
            })
            .await;
            written += w;
        }

        total = total.wrapping_add(n);

        if total % 64 < n || total % 64 == 0 {
            let irq = adapter::IRQ_COUNT.load(Ordering::Relaxed);
            let rx = adapter::RX_BYTES.load(Ordering::Relaxed);
            let wc = adapter::WAKE_CALLED.load(Ordering::Relaxed);
            let wh = adapter::WAKE_HIT.load(Ordering::Relaxed);

            sbi_puts("[echo] total=");
            sbi_puts(usize_str(total));
            sbi_puts(" irq=");
            sbi_puts(usize_str(irq));
            sbi_puts(" rx=");
            sbi_puts(usize_str(rx));
            sbi_puts(" wake=");
            sbi_puts(usize_str(wc));
            sbi_puts("/");
            sbi_puts(usize_str(wh));
            sbi_puts("\n");
        }
    }
}
