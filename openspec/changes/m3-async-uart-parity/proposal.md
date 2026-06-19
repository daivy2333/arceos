# M3: `examples/async_uart` — Async UART Parity PoC

> Project: arceos (`async-uart-1`)
> Created: 2026-06-19
> Status: active
> Analysis: `.claude/analysis/m3-async-uart-parity.md`
> Covers: T-012, T-013, T-014

## Why

M1 has RISC-V PLIC/IRQ 10 operational; M2 provides truly-blocking single-Future
`block_on`. The next step is to wire them together: build a self-contained
`examples/async_uart` that proves RX/TX copiers + SPSC ring + `axtask::block_on`
can deliver bidirectional echo on a real UART — the StarryOS parity checkpoint.

M3 does NOT replace SBI console, touch stdin/stdout, introduce TTY/POSIX
readiness, or generalize beyond single-UART/single-reader/single-writer.
Those belong to M5/M6.

## What Changes

- New `examples/async_uart` workspace member with `Cargo.toml` using:
  `axstd` (alloc + irq + multitask), `axtask`, `axhal`, `axconfig`, `kspin`,
  `memory_addr`, and `uart_16550` via path dependency with `async` feature.
- `examples/async_uart/src/adapter.rs`: `ArceOsRuntime` (spawn + block_on),
  `ArceOsWakerSet` (single-waker IRQ-safe slot), `ArceOsUartPort`, static rings,
  IRQ trampoline, bootstrap sequence.
- `examples/async_uart/src/main.rs`: echo Future using ring-aware wait semantics
  (double-check pop/push with waker registration), logging, idle gate verification.

## Impact

- **New capability**: async UART echo on RISC-V QEMU virt.
- **No regression**: SBI console unchanged; `examples/uart_irq` and `axtask` M2
  tests continue to build/pass.
- **Rollback**: delete workspace member entry + example directory.
