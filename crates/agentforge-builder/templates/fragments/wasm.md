## WASM-1. WebAssembly Targets

### WASM-1.1 Linear Memory Discipline
- Treat linear memory as scarce and allocation-costly. Prefer Rust-owned
  data structures; keep the JS/wasm boundary thin.
- Never hand raw pointers across the JS boundary. Use `wasm-bindgen`,
  `js_sys`, `web_sys`, or explicit offset+length pairs with bounds checks.
- Avoid `Vec`/`String` allocations in hot per-frame paths; reuse buffers.

### WASM-1.2 wasm-bindgen Interop
- Mark only what must cross the boundary `#[wasm_bindgen]` — nothing more.
- Accept/return JSON or `JsValue` sparingly; prefer structured types with
  `#[wasm_bindgen]` attributes over string-serialized ad-hoc payloads.
- Version the export surface like a public API: changing a signature is a
  breaking change for the host application.

### WASM-1.3 Async & JS FFI
- Never block on JS callbacks from wasm. Use `async` with promises via
  `wasm-bindgen-futures` and `spawn_local` for fire-and-forget tasks.
- Propagate errors across the boundary as typed values, not panics. A panic
  in wasm aborts the instance — there is no unwinding host safety net.

### WASM-1.4 Size & Startup Budgets
- Keep the wasm binary small: enable `opt-level = "s"` or `"z"` for release,
  strip debug symbols, and avoid pulling in the full std/allocator when a
  `no_std` target works.
- Measure gzipped size and startup time in CI, not just in local builds.

---

## WASM-2. Build & Tooling

### WASM-2.1 Target Selection
- Build for `wasm32-unknown-unknown` (browser) and `wasm32-wasi`
  (server/edge) explicitly; never assume one target covers both.
- Use `wasm-bindgen`/`wasm-pack` for browser output; keep the WIT/wasi
  interface declarative for wasm32-wasi.

### WASM-2.2 Testing on Target
- Run tests on the actual wasm target, not only native. Use
  `wasm-bindgen-test` for browser-flavored suites and WASI runtimes for the
  server target.
- Treat a test that passes natively but fails on wasm as a wasm bug, not a
  test bug.