# M3 Delta Spec: Async UART Parity

## ADDED Requirements

### REQ-M3-001: Async UART Echo
The system SHALL support interrupt-driven bidirectional echo on a single
MMIO UART on RISC-V QEMU virt using RX/TX copier tasks, SPSC ring buffers,
and `axtask::block_on`.

### REQ-M3-002: Blocking-on-Empty
When the RX ring is empty, the echo consumer SHALL block (not busy-wait)
and wake only when the RX copier pushes data.

### REQ-M3-003: Idle Without Polling
When no input is received for 5-10 seconds, IRQ, wake, and copier counters
SHALL remain steady, proving no yield-poll busy-loop.

### REQ-M3-004: SBI Console Preservation
The SBI console SHALL remain the output path for startup, panic, and debug
logs. The MMIO UART SHALL NOT replace or interfere with SBI output.

### REQ-M3-005: Rollback Isolation
Deleting the `examples/async_uart` workspace member and directory SHALL
fully revert all M3 changes without affecting M1 (PLIC), M2 (block_on),
or `examples/uart_irq`.

## MODIFIED Requirements

None. M3 adds a new example without modifying existing modules.

## REMOVED Requirements

None.
