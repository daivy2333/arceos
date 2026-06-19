# M3 Design: `examples/async_uart` Adapter Architecture

## File Layout

```
examples/async_uart/
├── Cargo.toml       # workspace member; uart_16550 path dep + async feature
└── src/
    ├── main.rs      # bootstrap, echo Future, verification logging
    └── adapter.rs   # Runtime / WakerSet / UartPort / static rings / ISR
```

## Adapter Design

### Runtime (`ArceOsRuntime`)

```rust
impl OsRuntime for ArceOsRuntime {
    fn spawn<F>(future: F, name: &str) { /* spawn_raw(move || block_on(future), ...) */ }
    fn block_on<F>(future: F) -> F::Output { axtask::block_on(future) }
}
```

- `spawn` maps to `axtask::spawn_raw(move || axtask::block_on(future), name, axconfig::TASK_STACK_SIZE)`.
- `block_on` delegates to the crate-root `axtask::block_on`.

### WakerSet (`ArceOsWakerSet`)

Single-waker IRQ-safe slot (not a general `PollSet`):
- `register(waker)`: stores/updates waker in `SpinRaw<Option<Waker>>`.
- `wake()`: takes waker, wakes, returns 0 or 1.

Documented boundary: M3 only has one consumer per ring. If two different
waiters need the same ring, the implementation must upgrade to bounded set
rather than silently overwrite. For M3, single-writer/single-reader is
sufficient.

### UART Ownership Chain

```
SpinNoIrq<Uart16550<MmioBackend>>  // exclusive register access
  ↓
ArceOsUartPort                     // &'static to the SpinNoIrq
  ↓
static CACHED_IER: AtomicU8        // shared between ISR and copier callbacks
  ↓
Once<Arc<AsyncUartDriver<...>>>    // pinned until kernel exit
  ↓
static mut RX_BUF/TX_BUF           // Once-init backing storage
```

### IRQ Trampoline

ArceOS handler type is `fn()` (no argument). A static trampoline binds:
- IRQ 10 (PLIC source)
- MMIO base VA
- `CACHED_IER` for IER restore
- Calls into `uart_isr_handler(10, base, &CACHED_IER)`.

No global IRQ hook — uses ArceOS `axhal::irq::register(10, handler)`.

### Bootstrap Order (Strict)

1. Parse MMIO VA, construct + init synchronous UART
2. Initialize RX/TX ring backing storage
3. Create + publish UART port and driver (`Arc<AsyncUartDriver>`)
4. Register IRQ 10 trampoline
5. Spawn RX/TX copier tasks
6. Enable PLIC source
7. Write UART IER to enable RX DATA_READY
8. Spawn echo Future

Invariant: when any device IRQ arrives, driver, copier wakers, and trampoline
context are all published. Early/panic logging stays on SBI console.

## Echo Future — Ring-Aware Wait Semantics

`embedded_io_async::Read` returns `Ok(0)` on empty RX ring; `Write` allows
short writes; `flush` returns immediately. These cannot directly implement
waiting echo. M3 uses local Futures around the public ring API:

**RX**: `pop → empty? → register(waker) → pop again → Ready(data) or Pending`
**TX**: `push → full? → register(waker) → push again → Ready(n) or Pending`

The echo task loops short writes until the current RX batch is fully consumed
into the TX ring. It is the sole consumer of the RX ring and sole producer
of the TX ring, satisfying the SPSC safety precondition.

## Risks & M3 Boundaries

| Risk | M3 Handling | Deferred To |
|------|-------------|-------------|
| TX copier tight loop on full ring | Gate observation; fix if busy-loop detected | M3/M6 |
| RX ring overflow → data loss | Bounded payload only | M4/M6 |
| Single-port AtomicWaker | Single UART only | M6 |
| IER RMW SMP race | SMP=1 only | M6 |
| Incomplete async read/write contract | Local Future bridge | M6 |

## Dependencies

- `ustd` (alloc + irq + multitask)
- `axtask`, `axhal`, `axconfig`, `kspin`, `memory_addr`
- `uart_16550` = `{ path = "../../../uart_16550", features = ["async"] }`
- No `axpoll`, TTY, VFS, POSIX.

## Verification Gate

### Build
- riscv64 release: build passes, SBI startup log preserved
- Local `uart_16550 0.6.0` resolved
- No `axpoll`/TTY/VFS/POSIX new dependencies

### Functional
- 1B, 64B, 4KiB input echoed in order, intact
- Logs/counters prove IRQ, RX copier, echo, TX copier all active
- No IRQ storm after handler return

### Idle
- No input: echo, RX copier, TX copier all Pending/Blocked
- Wake restores operation; no `yield_now` polling
- 10-min / 20-boot stability → M4

### Rollback
- Delete workspace member + example directory → full revert
- M1 platform, M2 block_on, `examples/uart_irq` unmodified
- SBI console behavior unchanged
