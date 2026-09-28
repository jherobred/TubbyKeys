# Contributing to TubbyKeys

Thanks for helping. Bug fixes, sound packs, features and docs are all welcome.

## Ways to help

- **Sound packs:** record your keyboard and add it to the marketplace. See [marketplace/README.md](marketplace/README.md).
- **Bugs:** open an issue with your Windows version, audio device and steps to reproduce.
- **Code:** pick an open issue, or open one first to discuss a bigger change.

## Setup

1. Install [Rust](https://rustup.rs), [Node.js](https://nodejs.org) 22 or newer, and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) (MSVC build tools and WebView2).
2. Clone the repo and run:

```bash
npm install
npm run tauri dev
```

The app starts in the tray. On first run it also opens the settings window.

## Project layout

| Path | What it is |
| --- | --- |
| `src-tauri/src/audio.rs` | Real-time mixer and the thread that owns the output stream |
| `src-tauri/src/keyboard.rs` | Global key listener (scan codes only) |
| `src-tauri/src/keymap.rs` | Scan code to row and stereo position |
| `src-tauri/src/packs.rs` | `pack.json` format, validation and decoding |
| `src-tauri/src/marketplace.rs` | Catalogue download, checksum checks and install |
| `src-tauri/src/lib.rs` | Tauri wiring: commands, tray, windows |
| `src-tauri/sounds/` | Built-in packs, compiled into the binary |
| `src/` | React UI for the settings window, tray pop-up and visualizer |
| `marketplace/` | Community pack catalogue |
| `scripts/marketplace.mjs` | Pack validator and `index.json` generator |

## Before you open a pull request

```bash
npm run typecheck
cd src-tauri
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the same checks on Windows.

## Ground rules

- **Privacy is the product.** Never log, store or send key identities or typed text. The key listener passes only scan-code positions to the audio engine and the visualizer.
- **The audio callback must stay real-time safe:** no locks, allocations, I/O or logging inside `Engine::process`.
- **Treat marketplace content as untrusted.** Validate paths, sizes and formats before touching the disk.
- Keep pull requests focused. One change per PR is easier to review.

By contributing, you agree that your contributions are licensed under the MIT license. Sound packs keep the license stated in their `pack.json`.
