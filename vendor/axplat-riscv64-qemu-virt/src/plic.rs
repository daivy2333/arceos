use core::num::NonZeroU32;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

use kspin::SpinNoIrq;
use riscv_plic::{Plic, PLICRegs};
use axplat::mem::{pa, phys_to_virt};

use crate::config::devices::PLIC_PADDR;

const PLIC_SOURCE_NUM: usize = 1024;
const ACTIVE_PRIORITY: u32 = 1;

static PLIC: SpinNoIrq<Option<Plic>> = SpinNoIrq::new(None);

/// Current hart's supervisor-mode PLIC context, set by `init_percpu`.
/// For SMP, each hart calls `init_percpu` before interrupts are enabled,
/// and the IRQ handler runs in NoPreempt context on its own hart.
static CURRENT_CONTEXT: AtomicUsize = AtomicUsize::new(0);

#[inline]
pub const fn s_mode_context(hart_id: usize) -> usize {
    hart_id * 2 + 1
}

#[inline]
pub fn current_context() -> usize {
    CURRENT_CONTEXT.load(Ordering::Relaxed)
}

pub fn init() {
    let base_vaddr = phys_to_virt(pa!(PLIC_PADDR)).as_usize();
    let base = NonNull::new(base_vaddr as *mut PLICRegs)
        .expect("PLIC base must be non-null");
    *PLIC.lock() = Some(unsafe { Plic::new(base) });
}

pub fn init_percpu(hart_id: usize) {
    let ctx = s_mode_context(hart_id);
    CURRENT_CONTEXT.store(ctx, Ordering::Relaxed);
    let mut guard = PLIC.lock();
    if let Some(p) = guard.as_mut() {
        p.init_by_context(ctx);
    }
}

pub fn set_enable(source: usize, ctx: usize, enabled: bool) {
    if source == 0 || source >= PLIC_SOURCE_NUM {
        warn!("PLIC ignore invalid source {}", source);
        return;
    }
    let Some(src) = NonZeroU32::new(source as u32) else {
        return;
    };
    let mut guard = PLIC.lock();
    let Some(p) = guard.as_mut() else {
        return;
    };
    if enabled {
        p.set_priority(src, ACTIVE_PRIORITY);
        p.enable(src, ctx);
    } else {
        p.disable(src, ctx);
        p.set_priority(src, 0);
    }
}

pub fn claim(ctx: usize) -> Option<NonZeroU32> {
    PLIC.lock().as_mut()?.claim(ctx)
}

pub fn complete(ctx: usize, source: NonZeroU32) {
    if let Some(p) = PLIC.lock().as_mut() {
        p.complete(ctx, source);
    }
}
