# ospivot

An AI engineering monorepo for research, inference, agents, training, data processing,
desktop applications and systems software.

## Repository map

| Path | Responsibility | Lifecycle |
|---|---|---|
| `apps/` | shippable applications / entry points (Tauri + React) | product |
| `crates/` | Rust workspace packages (systems / native capabilities) | source |
| `python/` | uv-managed Python workspace (ML / data / evaluation) | source |
| `node/` | pnpm-managed Node.js / TypeScript workspace (Agent Harness / Extensions / tooling) | source |
| `configs/` | configuration injection and registries | declarative |
| `data/` | datasets and transformed inputs | data |
| `models/` | model metadata, weights, adapters and caches | large / external |
| `artifacts/` | intentional outputs and reports | generated |
| `docs/` | architecture, research and AI handoff | durable knowledge |
| `tools/` | developer tooling | tooling |
| `tmp/` | disposable workspace | ephemeral |

## Language boundaries

Use the directory boundary as a default implementation rule:

- Rust → `crates/`
- Python → `python/`
- Node.js / TypeScript → `node/`
- React / Tailwind / Tauri application shell → `apps/`

`node/` is for headless Agent Harnesses, CLI tools, integrations, extensions and reusable tooling.
One-off checks and temporary experiments belong under `tmp/`, not under a source directory.

## Core commands

Rust workspace:

```text
cargo check --workspace
cargo test --workspace
```

Python workspace:

```text
cd python
uv sync
uv run pytest
```

Node / TypeScript workspace:

```text
pnpm install
pnpm node:typecheck
pnpm node:test
```

Frontend / Tauri desktop:

```text
pnpm desktop:dev
pnpm desktop:build
```

AI context packaging:

```text
npx repomix -c tools/repomix/1_.json
```

The resulting context bundle is disposable and belongs under `tmp/repomix/`.
