## TAURI-1. Desktop Shell

### TAURI-1.1 Core / Shell Separation
- Keep the Rust core platform-agnostic; push windowing, tray, and menu
  concerns to the Tauri layer. Never import `tauri` types into domain logic.
- Model the app as: domain crate (pure) ← commands layer ← Tauri runtime.

### TAURI-1.2 Frontend Build
- Treat the frontend as a build artifact: pin the framework version, commit
  the lockfile, and build it reproducibly (no network-only steps in CI).
- Bundle assets through Tauri's asset system; do not hardcode paths that
  break across dev/production (`devUrl` vs bundled assets).

### TAURI-1.3 Capabilities & Permissions
- Use the capability system (`capabilities/*.json`) with least privilege:
  grant only the commands and scopes the window actually needs.
- Never grant broad `shell:allow-execute` or wildcard asset scopes. Audit
  the permission files in the same review as `unsafe` code.

### TAURI-1.4 IPC Discipline
- Validate every command argument at the boundary; treat `Invoke` input as
  untrusted. Return typed `Result` from commands, never panic.
- Keep IPC payloads small and typed. Prefer IDs over serialized objects for
  cross-thread references.

---

## TAURI-2. Updates & Releases

### TAURI-2.1 Signing
- Sign updater artifacts with a real key pair in CI; keep the private key in
  secrets, never in the repo. Verify signatures on the update path.

### TAURI-2.2 Versioning
- Keep the frontend and Rust package versions in lockstep with the
  `tauri.conf.json` version; the updater compares against it, so drift
  silently breaks updates.