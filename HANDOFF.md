# OSPivot Engineering Handoff & State Record

## 1. Project Identity & Architecture
- **Name**: `ospivot`
- **Core Principle**: Purely Declarative, Zero-Guesswork, Bidirectional Cross-OS Transition Engine.
- **Contract Boundary**: All runtime variables MUST be explicitly declared in `configs/transition.toml`.

## 2. Transition Plan (Active)
- **Source**: Windows 11 (Host)
- **Target**: Linux Mint 22.3 Cinnamon 64-bit
- **ISO Path**: Configured via `configs/transition.toml` (Zero-scan policy enforced).
- **Disk Topology**: Target Disk 1 (500G NVMe, Erase) | Scratch Disk 0 (1TB WDC HDD, 8GB FAT32 Boot).
