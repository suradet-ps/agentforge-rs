## EMBED-1. no_std Firmware

### EMBED-1.1 no_std Discipline
- Keep the core crate `#![no_std]`; isolate `std`/allocator dependencies in
  a thin host-facing layer. Do not let `std` leak into driver code.
- Use `core::` and `alloc::` deliberately; document each `alloc` use site.

### EMBED-1.2 Peripheral Access
- Wrap hardware registers behind safe abstractions (one struct per
  peripheral), never expose raw MMIO pointers in the public API.
- Use `critical-section` or a static mutex discipline for shared
  peripherals; document the interrupt priority assumptions.

### EMBED-1.3 Interrupt & Concurrency Safety
- Every `unsafe` block touching hardware must carry a `// SAFETY:`
  comment explaining register ordering and interrupt masking.
- Keep interrupt handlers minimal: read → record → clear; defer work to the
  main loop or a scheduler.

### EMBED-1.4 Determinism & Budgets
- No dynamic allocation in hard-realtime paths. Profile flash and RAM
  budgets in CI (`cargo bloat`, link-size checks) and fail the build on
  regression.
- Pin the target triple (`thumbv7em-none-eabihf`, `riscv32imac-unknown-none-elf`, …)
  explicitly; never build for the host as the primary verification.