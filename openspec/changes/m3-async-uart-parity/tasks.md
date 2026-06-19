# M3 Tasks: `examples/async_uart`

> Covers: T-012 (RED build), T-013 (adapter/bootstrap), T-014 (echo + idle gate)

## Task 1 — T-012: RED Build Baseline

- [ ] 1.1 Create `examples/async_uart` directory with `Cargo.toml` and `src/main.rs`
- [ ] 1.2 Register `examples/async_uart` as workspace member in root `Cargo.toml`
- [ ] 1.3 `Cargo.toml`: `axstd = { features = ["alloc", "irq", "multitask"] }`
- [ ] 1.4 Add direct deps: `axtask`, `axhal`, `axconfig`, `kspin`, `memory_addr`
- [ ] 1.5 `uart_16550` = `{ path = "../../../uart_16550", features = ["async"] }`
- [ ] 1.6 Reference async API traits (`OsRuntime`, `OsWakerSet`, `RingBufRx`) → RED
- [ ] 1.7 Empty `main()` → GREEN compile check

**Gate**: `cargo check -p arceos-async-uart` passes. No `axpoll`/TTY/VFS/POSIX deps.
QA: `grep -r 'axpoll\|tty\|posix' examples/async_uart/Cargo.toml` returns empty.

---

## Task 2 — T-013a: Runtime & WakerSet Adapter

- [ ] 2.1 Implement `ArceOsRuntime`: `spawn` via `spawn_raw(|| block_on(future), name, TASK_STACK_SIZE)`
- [ ] 2.2 Implement `ArceOsRuntime`: `block_on` via `axtask::block_on(future)`
- [ ] 2.3 Implement IRQ-safe single-slot `ArceOsWakerSet` (`register` + `wake`)
- [ ] 2.4 Zero deps: no `yield_now` in adapter, no alloc in ISR path
- [ ] 2.5 Document single-waiter boundary in code comment

**Gate**: copier Future Pending → task blocks. Adapter has no polling loops.
QA: `cargo check -p arceos-async-uart`

---

## Task 3 — T-013b: UART Port, Static Storage & Trampoline

- [ ] 3.1 `ArceOsUartPort`: wraps `&'static SpinNoIrq<Uart16550<MmioBackend>>`
- [ ] 3.2 Once-initialized RX/TX backing storage (`static mut [u8; N]` rings)
- [ ] 3.3 `Arc<AsyncUartDriver>` via `Once<Arc<...>>` → never dropped
- [ ] 3.4 `CACHED_IER: AtomicU8` for ISR/copier callback sharing
- [ ] 3.5 IRQ trampoline: `fn()` → binds IRQ 10, MMIO VA, `CACHED_IER`
- [ ] 3.6 Per-phase atomic counters (IRQ dispatch, RX copier bytes, TX bytes)
- [ ] 3.7 Reuse `examples/uart_irq` constants: base=0x1000_0000, stride=1, IRQ=10

**Gate**: Before any IER enable, port/driver/rings/trampoline are published.
QA: `make A=examples/async_uart ARCH=riscv64 LOG=debug build`

---

## Task 4 — T-013c: Strict Bootstrap Sequence

- [ ] 4.1 Init MMIO UART (SBI output path unchanged)
- [ ] 4.2 Init RX/TX rings and `AsyncUartDriver`
- [ ] 4.3 Register IRQ 10 trampoline via `axhal::irq::register`
- [ ] 4.4 Spawn RX/TX copier tasks
- [ ] 4.5 Enable PLIC source via `axhal::irq::set_enable(10, true)`
- [ ] 4.6 Write UART IER to enable `DATA_READY`
- [ ] 4.7 Inject 1B → verify handler/copier counters change, no IRQ storm

**Gate**: Interrupt chain works; main loop does not drain RBR as copier workaround.
QA: `make A=examples/async_uart ARCH=riscv64 LOG=debug run` — verify copier count > 0.

---

## Task 5 — T-014a: Ring-Aware Echo (RED → GREEN)

- [ ] 5.1 Prove `embedded_io_async::Read` returns 0 on empty ring → RED record
- [ ] 5.2 RX Future: `pop → empty? → register(waker) → pop → Ready/Pending`
- [ ] 5.3 TX Future: `push → full? → register(waker) → push → Ready/Pending`
- [ ] 5.4 Echo loop: short-write handling, full RX batch → TX ring
- [ ] 5.5 Sole RX consumer / sole TX producer (SPSC safety)

**Gate**: 1B, 64B, 4KiB echo intact and in order. No lost wake or deadlock.
QA: Run with 1B/64B/4KiB input, verify output matches input exactly.

---

## Task 6 — T-014b: Idle, Negative & Rollback Gate

- [ ] 6.1 No input 5-10s → IRQ/wake/copier counters steady
- [ ] 6.2 Disable IRQ 10 → input not processed; re-enable → echo resumes
- [ ] 6.3 SBI startup/panic output unchanged
- [ ] 6.4 `examples/uart_irq` still builds (M1 regression check)
- [ ] 6.5 `axtask` M2 tests still pass
- [ ] 6.6 Document rollback: delete workspace member + directory

**Gate**: All scenarios in test matrix pass.
QA: Verify each scenario from the test matrix below.

### Test Matrix

| Scenario | Input | Expected |
|----------|-------|----------|
| Minimal | 1B | Single RX→TX echo |
| FIFO boundary | 64B | Exact order and length preserved |
| Fragmented | 4KiB | Multi-batch complete echo |
| Idle | 5-10s | No polling counter growth |
| IRQ masked | disable 10 + input | Copier not woken |
| Recovery | re-enable 10 | Subsequent input echoes |

---

## Task 7 — Closeout

- [ ] 7.1 Write run commands, log summaries, and limitations to change specs
- [ ] 7.2 Update `.claude/docs/tasks.md` (T-012/013/014 → done)
- [ ] 7.3 Update `.claude/docs/SNAPSHOT.md` with M3 completion
- [ ] 7.4 Preserve M4 reservations: 10-min stress, 20x boot, observability
- [ ] 7.5 Preserve M6 reservations: SPSC constraint, per-port waker, IER atomic, async API semantics
