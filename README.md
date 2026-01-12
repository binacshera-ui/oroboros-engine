# 🐍 OROBOROS

<div align="center">

![Rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)
![Bevy](https://img.shields.io/badge/Bevy-232326?style=for-the-badge&logo=bevy&logoColor=white)
![WebGPU](https://img.shields.io/badge/WebGPU-005A9C?style=for-the-badge&logo=webgl&logoColor=white)
![License](https://img.shields.io/badge/License-Proprietary-red?style=for-the-badge)

**A Voxel-Based Metaverse with DeFi-Survival Mechanics**

*"The risk is geometric"*

</div>

---

## 🎮 Overview

OROBOROS is an ambitious Metaverse project combining:
- **Voxel-based world** with procedural generation
- **High-performance Rust engine** using Bevy/WGPU
- **DeFi-Survival mechanics** - Play to Survive, not Play to Earn
- **Three interconnected realms** with unique gameplay

## 🌍 The Trinity Realms

| Realm | Theme | Gameplay |
|-------|-------|----------|
| 🏛️ **Neon Prime** | Cyberpunk City | Social / Tycoon - Trade & show off |
| 🌲 **Veridia** | Enchanted Forest | Survival / RPG - Craft & gather |
| 🔥 **Inferno** | Volcanic Hell | Hardcore PvP - Risk everything |

## 🛠️ Tech Stack

- **Language:** Rust (performance & safety first)
- **Engine:** Bevy + Custom Voxel Engine
- **Graphics:** WGPU (DX12/Vulkan/Metal)
- **Networking:** Custom UDP protocol (quinn/renet)
- **Blockchain:** EVM-compatible L2

## 📦 Project Structure

```
oroboros/
├── crates/           # Rust workspace crates
│   ├── oroboros/     # Main game client & server
│   ├── oroboros_core/
│   ├── oroboros_rendering/
│   ├── oroboros_procedural/
│   └── ...
├── assets/           # Game assets
├── config/           # Environment configs
├── docs/             # Documentation
└── infra/            # DevOps & infrastructure
```

## 🚀 Getting Started

### Prerequisites
- Rust 1.75+ (nightly recommended)
- Windows: Visual Studio Build Tools
- Linux: `libx11-dev`, `libasound2-dev`, `libudev-dev`

### Build & Run

```bash
# Build release
cargo build --release -p oroboros --bin oroboros_client

# Run client
cargo run --release -p oroboros --bin oroboros_client
```

## 📄 License

Proprietary - All Rights Reserved

---

<div align="center">

**Built with 🦀 Rust & ❤️ by the OROBOROS Team**

</div>
