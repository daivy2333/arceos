use core::future::Future;
use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use core::ptr::NonNull;
use core::task::Waker;

use alloc::boxed::Box;
use kspin::SpinNoIrq;

// ── critical-section implementation for embassy-hal-internal ──────────
//
// embassy-hal-internal's RingBuffer uses critical-section for atomic ops.
// ArceOS/RISC-V single-core: disable machine timer + external interrupts
// via sie register. SEIE=1 (bit 1, supervisor external), STIE=5 (bit 5,
// supervisor timer). We save/restore only SEIE — the ring buffer should
// not be accessed from timer interrupt context.

struct ArceOsCriticalSection;
critical_section::set_impl!(ArceOsCriticalSection);

unsafe impl critical_section::Impl for ArceOsCriticalSection {
    unsafe fn acquire() -> critical_section::RawRestoreState {
        let sie = riscv::register::sie::read();
        // Disable supervisor external interrupts (bit 9 in sie)
        unsafe { riscv::register::sie::clear_sext() };
        core::sync::atomic::compiler_fence(Ordering::SeqCst);
        sie.bits() as u8
    }

    unsafe fn release(token: critical_section::RawRestoreState) {
        core::sync::atomic::compiler_fence(Ordering::SeqCst);
        if token != 0 {
            unsafe { riscv::register::sie::set_sext() };
        }
    }
}

use uart_16550::os::{OsRuntime, OsWakerSet};
use uart_16550::async_::driver::{AsyncUartDriver, UartPort};
use uart_16550::async_::ring_buffer::{RingBufRx, RingBufTx};
use uart_16550::Uart16550;
use uart_16550::backend::MmioBackend;
use uart_16550::Config;

// ── QEMU virt platform constants ─────────────────────────────────────

pub const UART_PADDR: usize = 0x1000_0000;
pub const UART_STRIDE: u8 = 1;
pub const UART_IRQ: usize = 10;

// ── Counters (observability) ──────────────────────────────────────────

pub static IRQ_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static WAKE_CALLED: AtomicUsize = AtomicUsize::new(0);
pub static WAKE_HIT: AtomicUsize = AtomicUsize::new(0);
pub static RX_BYTES: AtomicUsize = AtomicUsize::new(0);

// ── OsRuntime adapter ─────────────────────────────────────────────────

pub struct ArceOsRuntime;

impl OsRuntime for ArceOsRuntime {
    fn spawn<F>(future: F, name: &str)
    where
        F: Future + Send + 'static,
        F::Output: Send,
    {
        axtask::spawn_raw(
            move || {
                let _ = axtask::block_on(future);
            },
            name.into(),
            axconfig::TASK_STACK_SIZE,
        );
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        axtask::block_on(future)
    }
}

// ── OsWakerSet adapter (single-waker slot) ────────────────────────────

pub struct ArceOsWakerSet {
    waker: SpinNoIrq<Option<Waker>>,
}

unsafe impl Send for ArceOsWakerSet {}
unsafe impl Sync for ArceOsWakerSet {}

impl OsWakerSet for ArceOsWakerSet {
    fn new() -> Self {
        Self {
            waker: SpinNoIrq::new(None),
        }
    }

    fn register(&self, waker: &Waker) {
        *self.waker.lock() = Some(waker.clone());
    }

    fn wake(&self) -> u32 {
        WAKE_CALLED.fetch_add(1, Ordering::Relaxed);
        let taken = self.waker.lock().take();
        match taken {
            Some(w) => {
                WAKE_HIT.fetch_add(1, Ordering::Relaxed);
                w.wake();
                1
            }
            None => 0,
        }
    }
}

// ── UartPort adapter ──────────────────────────────────────────────────

pub struct ArceOsUartPort {
    inner: SpinNoIrq<Uart16550<MmioBackend>>,
}

impl ArceOsUartPort {
    pub unsafe fn new_mmio(base: NonNull<u8>, stride: u8) -> Self {
        let uart = unsafe {
            Uart16550::new_mmio(base, stride)
                .expect("UART MMIO init must succeed on QEMU virt")
        };
        Self {
            inner: SpinNoIrq::new(uart),
        }
    }

    pub fn init(&self, config: Config) {
        self.inner.lock().init(config).expect("UART config init must succeed");
    }

    pub fn set_ier_rx_data_ready(&self) {
        let mut uart = self.inner.lock();
        let ier_val = uart.ier();
        uart.set_ier(ier_val | uart_16550::spec::registers::IER::DATA_READY);
    }
}

impl UartPort for ArceOsUartPort {
    fn receive_bytes(&self, buf: &mut [u8]) -> usize {
        let n = self.inner.lock().receive_bytes(buf);
        if n > 0 {
            RX_BYTES.fetch_add(n, Ordering::Relaxed);
        }
        n
    }

    fn send_bytes(&self, buf: &[u8]) -> usize {
        self.inner.lock().send_bytes(buf)
    }
}

// ── Static storage ────────────────────────────────────────────────────

pub static CACHED_IER: AtomicU8 = AtomicU8::new(0);

static UART_BASE: AtomicUsize = AtomicUsize::new(0);

// ── SBI helpers (diagnostic) ──────────────────────────────────────────

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

fn hex(mut n: usize) -> &'static str {
    const CAP: usize = 18;
    static mut B: [u8; CAP] = [0; CAP];
    unsafe {
        if n == 0 { B[0] = b'0'; return core::str::from_utf8_unchecked(&B[..1]); }
        let mut i = CAP;
        while n > 0 && i > 0 { i -= 1; B[i] = if (n & 0xf) < 10 { b'0' + (n & 0xf) as u8 } else { b'a' + (n & 0xf) as u8 - 10 }; n >>= 4; }
        if i >= 2 { i -= 2; B[i] = b'x'; B[i+1] = b'0'; }
        core::str::from_utf8_unchecked(&B[i..])
    }
}

// ── IRQ trampoline ────────────────────────────────────────────────────

pub fn irq_trampoline() {
    IRQ_COUNT.fetch_add(1, Ordering::Relaxed);

    let base = UART_BASE.load(Ordering::Relaxed);
    if base == 0 { return; }
    let Some(base_ptr) = NonNull::new(base as *mut u8) else { return; };

    let pre_isr = unsafe { core::ptr::read_volatile((base + 2) as *const u8) };
    let pre_lsr = unsafe { core::ptr::read_volatile((base + 5) as *const u8) };
    let pre_ier = CACHED_IER.load(Ordering::Relaxed);

    uart_16550::async_::isr::uart_isr_handler(UART_IRQ, base_ptr, &CACHED_IER);

    let post_ier = CACHED_IER.load(Ordering::Relaxed);

    if pre_isr & 0x0E != 0 {
        sbi_puts("[irq] ISR="); sbi_puts(hex(pre_isr as usize));
        sbi_puts(" LSR="); sbi_puts(hex(pre_lsr as usize));
        sbi_puts(" IER:"); sbi_puts(hex(pre_ier as usize));
        sbi_puts("->"); sbi_puts(hex(post_ier as usize));
        sbi_puts(" rx="); sbi_puts(hex(RX_BYTES.load(Ordering::Relaxed)));
        sbi_puts(" wake="); sbi_puts(hex(WAKE_CALLED.load(Ordering::Relaxed)));
        sbi_puts("/"); sbi_puts(hex(WAKE_HIT.load(Ordering::Relaxed)));
        sbi_puts("\n");
    }
}

// ── Copier callbacks ──────────────────────────────────────────────────

fn enable_rx_intr() {
    let ier = CACHED_IER.load(Ordering::Relaxed);
    let new_val = ier | uart_16550::spec::registers::IER::DATA_READY.bits();
    CACHED_IER.store(new_val, Ordering::Relaxed);
    let base = UART_BASE.load(Ordering::Relaxed);
    if base != 0 {
        unsafe { core::ptr::write_volatile((base + 1) as *mut u8, new_val); }
    }
}

fn enable_tx_intr() {
    let ier = CACHED_IER.load(Ordering::Relaxed);
    let new_val = ier | uart_16550::spec::registers::IER::THR_EMPTY.bits();
    CACHED_IER.store(new_val, Ordering::Relaxed);
    let base = UART_BASE.load(Ordering::Relaxed);
    if base != 0 {
        unsafe { core::ptr::write_volatile((base + 1) as *mut u8, new_val); }
    }
}

// ── Bootstrap ─────────────────────────────────────────────────────────

pub type Driver = AsyncUartDriver<ArceOsRuntime, ArceOsWakerSet, ArceOsUartPort>;

use core::sync::atomic::AtomicBool;

static BOOTSTRAP_DONE: AtomicBool = AtomicBool::new(false);
use embassy_hal_internal::atomic_ring_buffer::RingBuffer;
static RX_RING: RingBuffer = RingBuffer::new();
static TX_RING: RingBuffer = RingBuffer::new();
const RING_CAP: usize = 4096;
static mut RX_BUF: [u8; RING_CAP] = [0; RING_CAP];
static mut TX_BUF: [u8; RING_CAP] = [0; RING_CAP];

static mut UART_PORT: *const ArceOsUartPort = core::ptr::null();
static mut DRIVER: *const Driver = core::ptr::null();

pub fn init_uart() -> &'static Driver {
    if BOOTSTRAP_DONE.load(Ordering::Acquire) {
        return unsafe { &*DRIVER };
    }

    use axhal::mem::phys_to_virt;
    use memory_addr::pa;

    let uart_vaddr = phys_to_virt(pa!(UART_PADDR)).as_usize();
    UART_BASE.store(uart_vaddr, Ordering::Relaxed);

    // 1. Initialize MMIO UART
    let port = unsafe {
        ArceOsUartPort::new_mmio(NonNull::new(uart_vaddr as *mut u8).unwrap(), UART_STRIDE)
    };
    port.init(Config::default());
    unsafe {
        core::ptr::write_volatile((uart_vaddr + 2) as *mut u8, 0x01); // FCR: FIFO+trigger=1
    }

    let port: &'static ArceOsUartPort = Box::leak(Box::new(port));
    unsafe { UART_PORT = port as *const ArceOsUartPort };

    // 2. Initialize ring buffers (must call init() to set backing storage)
    unsafe {
        RX_RING.init(core::ptr::addr_of_mut!(RX_BUF) as *mut u8, RING_CAP);
        TX_RING.init(core::ptr::addr_of_mut!(TX_BUF) as *mut u8, RING_CAP);
    }
    let rx_ring = unsafe { RingBufRx::new(&RX_RING) };
    let tx_ring = unsafe { RingBufTx::new(&TX_RING) };

    // 3. Create driver
    let driver = Driver::new(rx_ring, tx_ring, port);

    let driver: &'static Driver = Box::leak(Box::new(driver));
    unsafe { DRIVER = driver as *const Driver };

    // 4. Register IRQ 10
    axhal::irq::register(UART_IRQ, irq_trampoline);

    // 5. Spawn copier tasks
    driver.start_rx_copier(enable_rx_intr);
    driver.start_tx_copier(enable_tx_intr);

    // 6. Enable PLIC source
    axhal::irq::set_enable(UART_IRQ, true);

    // 7. Enable UART RX interrupt
    port.set_ier_rx_data_ready();

    BOOTSTRAP_DONE.store(true, Ordering::Release);
    driver
}
