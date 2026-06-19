# M1 PLIC Baseline Design

> Spec for `m1-riscv-plic-baseline` change, tasks T2.1-T4.5
> Branch: `async-uart-1`
> Date: 2026-06-19
> Approved: 2026-06-19 (brainstorming design OK)

## Purpose

Replace vendor `axplat-riscv64-qemu-virt 0.4.1` PLIC TODO with a complete supervisor-mode PLIC implementation: priority, enable/disable, per-hart context threshold, claim, dispatch, complete. Acceptance is `examples/uart_irq` probe (already RED-passing on T1.3) turning GREEN end-to-end on host byte input, with no IRQ storm and no SBI console regression on `examples/shell`.

## Scope

**In scope:**
- Add `riscv_plic = "0.2"` to vendor `axplat-riscv64-qemu-virt`
- New module `vendor/axplat-riscv64-qemu-virt/src/plic.rs`
- Modify `vendor/axplat-riscv64-qemu-virt/src/irq.rs` so `set_enable`/`register`/`handle(S_EXT)` route through PLIC
- Wire `init_percpu(cpu_id)` to set current hart's supervisor context threshold to 0
- Build verification: `SMP=1` and `SMP=4` for `examples/uart_irq`
- GREEN verification: existing `examples/uart_irq` probe shows `count > 0` and `rx > 0` after host byte injection
- Boundary tests: disable/re-enable, register/unregister, spurious trap, no-input idle
- SBI console regression on `examples/shell`
- Non-RISC-V build matrix unchanged (x86_64/aarch64/loongarch64)
- Two-stage review + documented rollback command

**Out of scope:**
- Async UART, Future executor, TTY, POSIX (M3/M5)
- Modifications to `axhal` or `axdriver`
- Introduction of `riscv-peripheral` / `ax-riscv-plic` / other PLIC crates
- Other platform crates (`x86_64`, `aarch64`, `loongarch64`, `armv7a`)

## Architecture

```
vendor/axplat-riscv64-qemu-virt/
├── Cargo.toml.orig           # + riscv_plic = "0.2"  (one new line)
├── Cargo.toml                # cargo regenerates from .orig
└── src/
    ├── lib.rs                # + mod plic;
    ├── irq.rs                # set_enable/register/handle route to plic.rs
    └── plic.rs               # NEW: PLIC init, per-hart context, claim/complete
```

Dependencies:
- Root `Cargo.toml`: no change
- Root `Cargo.lock`: cargo updates automatically when vendor `Cargo.toml.orig` adds `riscv_plic`
- `[patch.crates-io]`: unchanged (only `axplat-riscv64-qemu-virt` is patched)

External crate:
- `riscv_plic 0.2.0` from crates.io — provides `Plic::new`, `set_priority`, `enable/disable`, `set_threshold`, `claim -> Option<NonZeroU32>`, `complete(ctx, source)`, `init_by_context(ctx)`

## Components

### `plic.rs` (new)

```rust
//! RISC-V PLIC wrapper for axplat-riscv64-qemu-virt.
//!
//! Wraps `riscv_plic::Plic` in `SpinNoIrq` and provides the supervisor-mode
//! context calculation for QEMU virt (`hart_id * 2 + 1`). Source 0 is reserved
//! by PLIC spec and ignored on enable/disable/claim.

#![cfg(feature = "irq")]

use core::num::NonZeroU32;
use core::ptr::NonNull;

use riscv::register::mhartid;
use riscv_plic::{Plic, PLICRegs};

use crate::config::devices::PLIC_PADDR;
use crate::mem::virt_to_phys;  // not used directly; phys_to_virt is via MemIf

const PLIC_BASE_VADDR: usize = axplat::mem::phys_to_virt(pa!(PLIC_PADDR)).as_usize();
const PLIC_SOURCE_NUM: usize = 1024;
const ACTIVE_PRIORITY: u32 = 1;

static PLIC: SpinNoIrq<Option<Plic>> = SpinNo_irq::new(None);

#[inline]
pub const fn s_mode_context(hart_id: usize) -> usize { hart_id * 2 + 1 }

#[inline]
fn current_hart_id() -> usize { mhartid::read().bits() }

pub fn init() {
    let base = NonNull::new(PLIC_BASE_VADDR as *mut PLICRegs)
        .expect("PLIC base must be non-null");
    let p = unsafe { Plic::new(base) };
    // All sources start at priority 0 (disabled by spec).
    // Per-context threshold is set in init_percpu.
    *PLIC.lock() = Some(p);
}

pub fn init_percpu(_cpu_id: usize) {
    let mut guard = PLIC.lock();
    if let Some(p) = guard.as_mut() {
        p.init_by_context(s_mode_context(current_hart_id()));
    }
}

pub fn set_enable(source: usize, ctx: usize, enabled: bool) {
    if source == 0 || source >= PLIC_SOURCE_NUM {
        warn!("PLIC ignore invalid source {}", source);
        return;
    }
    let Some(src) = NonZeroU32::new(source as u32) else { return; };
    let mut guard = PLIC.lock();
    let Some(p) = guard.as_mut() else { return; };
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
```

**Note on Hart ID**: QEMU virt's `mhartid` matches the logical hart ID used by SBI / OpenSBI, and matches the context calculation `hart_id * 2 + 1`. This is consistent with the StarryOS 0.3.1-pre.6 port.

### `irq.rs` changes

`IrqIfImpl` already exists. The three methods get rewritten:

- `set_enable(irq, enabled)`:
  - CPU-side cause (S_TIMER, S_SOFT, S_EXT) → no-op (PLIC does not control)
  - device IRQ → `plic::set_enable(irq, s_mode_context(current_hart_id()), enabled)`

- `register(irq, handler)`:
  - S_TIMER / S_SOFT: same as today (TIMER_HANDLER / IPI_HANDLER compare-exchange)
  - S_EXT: warn + return false (kept from today; this is the S_EXT-as-scause case, not used after PLIC is implemented)
  - EX_IRQ: register handler in table, then `Self::set_enable(irq, true)` (preserves today)

- `handle(irq)`:
  - S_TIMER: unchanged
  - S_SOFT: unchanged (clears_ssoft)
  - S_EXT (the live path with PLIC): claim from current hart's context; if `Some(src)`, look up handler in `IRQ_HANDLER_TABLE`, call it, then `plic::complete(ctx, src)`; if `None`, warn "spurious S_EXT"
  - EX_IRQ: unreachable (today)

`current_hart_id()` is implemented as `riscv::register::mhartid::read().bits()` (matches SBI/OpenSBI hart ID assignment, consistent with QEMU virt's `hart_id * 2 + 1` context formula).

`init_percpu` in `irq.rs` already calls `sie::set_sext()` which remains. After PLIC is initialized in `init_later`, `init_percpu` also calls `plic::init_percpu(cpu_id)`.

### `axconfig.toml` change

Add to `[devices]` in **both** `vendor/axplat-riscv64-qemu-virt/axconfig.toml` and `configs/defconfig.toml`:

```toml
# PLIC base physical address (QEMU virt: 0x0c00_0000, size 0x21_0000).
plic-paddr = 0x0c00_0000
```

This makes `crate::config::devices::PLIC_PADDR` available via `axconfig_macros::include_configs!`.

### `init.rs` changes

`init_later` and `init_later_secondary` add `crate::plic::init_percpu(_cpu_id)` after `crate::irq::init_percpu()`. The PLIC instance itself must be initialized once before any hart enters. Two options:

1. Initialize PLIC in `init_early` (BSP only)
2. Initialize PLIC lazily on first `init_percpu` if not yet initialized

Choose **Option 1** — initialize once in `init_early` on cpu_id == 0. Simpler, deterministic.

### `lib.rs` change

```rust
mod plic;  // NEW
```

## Data Flow (UART RX IRQ 10, full round trip)

```
host byte → UART THR → UART IER.DATA_READY asserts IRQ 10 line
                              │
                              ▼
PLIC: priority(10) = 1 > threshold(0) & enable[10][ctx] = 1
                              │
                              ▼
PLIC asserts S_EXT to current hart
                              │
                              ▼
hart trap → axhal::irq dispatch → IrqIfImpl::handle(S_EXT)
                              │
                              ▼
plic::claim(s_mode_context(current_hart_id())) → Some(NonZeroU32(10))
                              │
                              ▼
IRQ_HANDLER_TABLE.handle(10) → irq10_handler()
   ├── read IIR (volatile)
   ├── loop { read(LSR); if DATA_READY: read(RBR); count++ }
   └── RX_BYTE_COUNT += drained
                              │
                              ▼
plic::complete(ctx, source 10)
                              │
                              ▼
mret → main loop reads counters via SBI
```

## Error Handling

| Condition | Behavior |
|-----------|----------|
| `source == 0` (set_enable) | warn + early return; preserve PLIC "no interrupt" semantics |
| `source >= 1024` (set_enable) | warn + early return; defensive |
| `PLIC` static not yet initialized | `set_enable` / `complete` early return; `claim` returns None |
| `claim()` returns None on S_EXT | log warn "spurious S_EXT"; do NOT call complete (None has no source) |
| Handler table lookup returns false | current behavior: warn "Unhandled IRQ n" |
| SpinNoIrq lock contention | Impossible: PLIC MMIO access only in IRQ context + BSP init; no SMP concurrency on PLIC MMIO |

## Concurrency

- All PLIC MMIO access under `SpinNoIrq` (already used elsewhere in vendor axplat)
- Handler runs in `NoPreempt` IRQ context; PLIC lock acquired for the full claim → dispatch → complete sequence
- The existing `IRQ_HANDLER_TABLE: HandlerTable<MAX_IRQ_COUNT>` already provides per-IRQ handler storage; no change

## Testing & Verification

| Task | Verification | Pass criterion |
|------|--------------|----------------|
| T2.1 | `make A=examples/uart_irq ARCH=riscv64 build` | 0 warnings |
| T2.1 | `make A=examples/uart_irq ARCH=riscv64 SMP=4 build` | 0 warnings |
| T2.2 | Boot QEMU, observe "[plic] init_by_context ctx=N" log | BSP ctx=1; AP ctx=3,5,7 |
| T3.1 | Inject host byte into uart_irq probe | `count > 0`, `rx > 0`, no panic |
| T3.2 | uart_irq with set_enable(10, false) before inject | counter stable; re-enable restores |
| T3.3 | register collision + unregister + spurious | no panic, no storm |
| T4.1 | `make A=examples/shell ARCH=riscv64 BLK=y APP_FEATURES=use-ramfs run` | SBI console + shell prompt |
| T4.2 | T3.1 looped 10× with 1B and 64B | counts match |
| T4.3 | T3.1 idle 30s | no spurious loop |
| T4.4 | `make ARCH=x86_64 build` + aarch64 + loongarch64 | no new regression |
| T4.5 | two-stage review + `cargo update -p axplat-riscv64-qemu-virt` rehearsal | no Critical/Important; rollback reproducible |

Each gate task produces a log file under `/tmp/m1-t*-{date}.log` and updates `openspec/changes/m1-riscv-plic-baseline/tasks.md`.

## Key File Changes Summary

| File | Change | Approx LOC |
|------|--------|-----------|
| `vendor/axplat-riscv64-qemu-virt/Cargo.toml.orig` | + `riscv_plic = "0.2"` dep | +1 |
| `vendor/axplat-riscv64-qemu-virt/axconfig.toml` | + `[devices] plic-paddr = 0x0c00_0000` | +1 |
| `configs/defconfig.toml` | + `[devices] plic-paddr = 0x0c00_0000` (mirrored) | +1 |
| `vendor/axplat-riscv64-qemu-virt/src/plic.rs` | NEW module | ~120 |
| `vendor/axplat-riscv64-qemu-virt/src/irq.rs` | rewrite set_enable/register/handle to route through plic | ~50 changed |
| `vendor/axplat-riscv64-qemu-virt/src/init.rs` | + plic::init in init_early + plic::init_percpu in init_later/init_later_secondary | ~6 changed |
| `vendor/axplat-riscv64-qemu-virt/src/lib.rs` | + mod plic | +1 |
| Root `Cargo.lock` | auto-update | (no manual) |
| `examples/uart_irq/src/main.rs` | unchanged (already GREEN-ready from T1.3) | 0 |
| `openspec/changes/m1-riscv-plic-baseline/tasks.md` | T2.x → [x] as completed | ~20 |
| `.claude/docs/SNAPSHOT.md` | sync progress | +5 |
| `.claude/docs/tasks.md` | sync M1 row | +3 |

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `riscv_plic 0.2.0` API mismatch | Read API locally first; spec already covers all needed methods |
| SMP AP context init deadlock | PLIC is initialized in `init_early` (BSP only); AP `init_percpu_secondary` only sets threshold |
| Spurious S_EXT after UART byte + RBR read | handler drains RBR fully; level condition cleared before complete |
| PLIC lock held across SBI console (yield) | No — handler in IRQ context, SBI call only in main loop; PLIC lock never crosses SBI |
| New dependency breaks existing build | `make ARCH=x86_64 build` etc. matrix test in T4.4; riscv_plic is no_std and only used inside riscv64 vendor |

## Out of Scope (explicit)

- Async UART, Future executor, TTY, POSIX (M3/M5 work)
- `axhal` or `axdriver` modifications
- Alternative PLIC crates (`riscv-peripheral`, `ax-riscv-plic`)
- Other platform crates (`x86_64`, `aarch64`, `loongarch64`, `armv7a`)
- Performance optimization (e.g., batched claim, MSI) — M6 territory
- Device-tree parsing for PLIC source count — QEMU virt has fixed 1024 sources, hardcoded constant

## Acceptance

This spec is complete when:
1. T2.1-T4.5 all marked [x] in `openspec/changes/m1-riscv-plic-baseline/tasks.md`
2. All QEMU verification logs saved to `/tmp/m1-t*.log`
3. Two-stage review (spec compliance + code quality) produces no Critical/Important issues
4. `cargo update -p axplat-riscv64-qemu-virt` rollback command documented and tested
5. Spec archived via `openspec archive m1-riscv-plic-baseline`