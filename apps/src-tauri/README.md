# Tauri Runtime Boundary

Tauri is the application boundary, not the domain layer.

- `src/`: application commands, lifecycle and Tauri integration.
- `capabilities/`: Tauri permission/capability definitions.
- `icons/`: packaged application assets.
- shared Rust logic belongs in the root Cargo workspace under `crates/`.
