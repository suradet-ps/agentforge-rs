## BEVY-1. ECS Architecture

### BEVY-1.1 System Discipline
- Prefer small, single-responsibility systems with explicit `Query` and
  `Res` bounds. Avoid mega-systems that touch most of the world each frame.
- Order systems by dependency, not by registration accident. Use
  explicit `chain()` and labels when ordering matters.

### BEVY-1.2 Component Design
- Keep components plain data: `#[derive(Component)]` structs with public
  fields, no heavy logic. Push behavior into systems.
- Use enum + single-component patterns over many near-empty marker
  components unless archetype optimization demands otherwise.

### BEVY-1.3 Asset Pipeline
- Load assets through the `AssetServer` and handle `Handle` lifetimes
  explicitly; do not clone handles into long-lived state without a reason.
- Prefer `bevy_asset` typed loaders over ad-hoc file parsing in systems.

### BEVY-1.4 State & Events
- Model app flow with Bevy `State`/`States` transitions, not global flags.
  Use `EventWriter`/`EventReader` for one-shot communication; avoid polling
  world state every frame.
- Respect app lifecycle: clean up resources on state exit (despawn
  entities, unload assets) to avoid leaks across scene reloads.