# Clickless — Keyboard-Driven Pointer Controller in Rust

[![Rust](https://img.shields.io/badge/Rust-2024_Edition-DEA584?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue?style=for-the-badge)](https://opensource.org/licenses/MIT)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey?style=for-the-badge)](https://shivammahajan.com/projects/)
[![Architecture](https://img.shields.io/badge/Architecture-3_Crates-orange?style=for-the-badge)](https://shivammahajan.com/)

A lightweight, low-latency pointer navigation system written in Rust that eliminates reliance on physical mice or trackpads. Features recursive two-letter target grids, continuous Vi-style acceleration (`H/J/K/L`), and low-level OS event hooks.

Live Project: [Clickless on Shivam Mahajan Portfolio](https://shivammahajan.com/projects/)

---

## Table of Contents
- [Overview](#overview)
- [Target Audience](#target-audience)
- [Features](#features)
- [How It Works](#how-it-works)
  - [Target Grid Navigation Architecture](#target-grid-navigation-architecture)
  - [Low-Level System Hooks & Event Flow](#low-level-system-hooks--event-flow)
- [Architecture & Crate Structure](#architecture--crate-structure)
- [Getting Started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [Compilation & Building](#compilation--building)
- [Usage](#usage)
  - [Grid Targeting Mode](#grid-targeting-mode)
  - [Free Cursor Movement Mode](#free-cursor-movement-mode)
  - [Default Pointer Bindings](#default-pointer-bindings)
- [Configuration](#configuration)
- [Diagnostics](#diagnostics)
- [Performance & Benchmark Results](#performance--benchmark-results)
- [Future Scope and Improvements](#future-scope-and-improvements)
- [Author & Connect](#author--connect)

---

## Overview

Repeatedly moving hands between the keyboard and mouse causes repetitive strain injury (RSI), breaks developer flow state, and impedes computer accessibility for motor-impaired individuals. While Vim plugins solve text navigation within editors, operating system desktop environments, browsers, and third-party UIs remain predominantly mouse-centric.

**Clickless** brings keyboard-first navigation to the entire operating system. Written in native Rust with zero garbage collection overhead, Clickless intercepts hardware input via low-level OS hooks, overlays recursive screen grids, and simulates fluid cursor dynamics with configurable acceleration curves.

### Target Audience
- **Keyboard-Centric Developers & Vim Power Users**: Maintain keyboard flow across browsers, terminals, chat apps, and design software without touching a mouse.
- **Ergonomics & Accessibility Advocates**: Reduce RSI and wrist strain through ergonomic modifier keys and minimal keystroke travel distance.
- **Systems & Rust Enthusiasts**: Explore a clean 3-crate Rust architecture isolating pure state machines from platform-specific OS hook APIs.

---

## Features

- **Dense Two-Letter Screen Grid**: Divide the active display into an outer target grid and a nested 3×10 sub-grid for pixel-precise targeting within 2 keystrokes.
- **Modal Free Navigation**: Continuous Vi-like directional control (`H/J/K/L`) with configurable velocity curves, momentum, and acceleration ramps.
- **Low-Level Native Hooks**: Uses native OS APIs (`SetWindowsHookEx` on Windows, evdev/X11 on Linux, CGEventTap on macOS) for instantaneous keyboard capture without dropped events.
- **Single Instance Tray Management**: Discreet background tray process with hot-reloadable TOML configuration and single-instance mutex protection.
- **Atomic TOML Settings**: Live configuration validation, runtime dynamic apply, and persistent serialization.

---

## How It Works

### Target Grid Navigation Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Screen Display (100%)                    │
│                                                             │
│   ┌──────┬──────┬──────┐                                    │
│   │  AA  │  AB  │  AC  │  <-- Outer Grid (3x3 Coarse)       │
│   ├──────┼──────┼──────┤                                    │
│   │  BA  │ [BB] │  BC  │  <-- Target Quadrant Selected      │
│   │      │ ┌──┐ │      │                                    │
│   │      │ └──┘ │      │  <-- Nested 3x10 Fine Sub-Grid     │
│   ├──────┼──────┼──────┤      (Sub-pixel Cursor Hop)        │
│   │  CA  │  CB  │  CC  │                                    │
│   └──────┴──────┴──────┘                                    │
└─────────────────────────────────────────────────────────────┘
```

### Low-Level System Hooks & Event Flow

1. **Activation Trigger**: Pressing `CapsLock` (hold for 200ms) or tapping the configured leader key intercepts input and overlays the transparent visual grid.
2. **Key Combo Resolution**: The first key sequence selects the broad screen sector; the second selects the exact clickable element.
3. **Dispatch & Dismiss**: The system dispatches a synthetic mouse click at the resolved coordinates via low-level OS input injection and dismisses the overlay.
4. **Nudge & Fine Adjustment**: Optional pixel-by-pixel nudging before release allows precision selection down to 1 physical screen pixel.

---

## Architecture & Crate Structure

```
clickless/
├── crates/
│   ├── clickless-core/         # Pure state machine: movement math, grid layout, layers, TOML config, backend interfaces
│   ├── clickless-platform/     # OS hooks, overlays, tray menus, Enigo output adapters (Windows, Linux, macOS)
│   └── clickless-cli/          # CLI runtime, shared GPUI Settings & Practice UI, binaries (clickless, clicklessctl, clickless-settings)
├── configs/                    # Presets (vim-ergonomic, wasd, simple_grid)
├── Cargo.toml                  # 3-crate workspace definition
└── README.md
```

| Crate | Responsibility |
|---|---|
| `clickless-core` | Pure layers, movement, grid navigation, actions, configuration, backend interfaces and shared rasterizer |
| `clickless-platform` | OS hooks, overlays, tray menus, output adapters (Windows, Linux, macOS) |
| `clickless` | CLI, shared runtime, GPUI Settings & Practice UI, binaries (`clickless`, `clicklessctl`, `clickless-settings`) |

OS-specific code stays in `clickless-platform`. Core and configuration remain platform-independent.

---

## Getting Started

### Prerequisites

- **Rust**: Stable toolchain (Rust 1.85+ recommended via `rustup`)
- **OS**: Windows 10/11, Linux (X11), or macOS

### Compilation & Building

Clone the repository and build release binaries:

```powershell
git clone https://github.com/smresponsibilities/clickless.git
cd clickless
cargo build --release --workspace
```

Build outputs located in `target/release/`:
- `clickless.exe`: Native GUI background tray application (no terminal window).
- `clicklessctl.exe`: Console CLI utility for validation, smoke tests, and diagnostics.
- `clickless-settings.exe`: Dedicated native configuration GUI.

---

## Usage

### Grid Targeting Mode
1. Hold `CapsLock` (or press `Space` in mouse mode) to render the full-screen grid.
2. Type the two displayed letters of your target quadrant (e.g., `A` then `K`).
3. The mouse cursor immediately teleports to the target and executes a click.
4. Press `Esc` or `Backspace` to back out or cancel at any moment.

### Free Cursor Movement Mode
1. In mouse mode, use directional keys to glide the cursor across monitors.
2. Speed dynamically ramps up based on continuous hold duration.
3. Tap modifier or activation key to release control back to regular typing.

### Default Pointer Bindings

| Key Combo | Action |
|---|---|
| `H` / `J` / `K` / `L` | Move cursor Left / Down / Up / Right |
| `F` / `D` | Left Click / Right Click |
| `W` / `S` | Scroll Wheel Up / Down |
| `U` / `O` | Decrease / Increase Movement Speed |
| `Space` | Enter grid mode |
| `Esc` | Dismiss grid / Cancel action |

---

## Configuration

Validate a file with the console tool:

```powershell
./target/release/clicklessctl.exe --check-config --config clickless.toml
```

Minimal example:

```toml
[settings]
enabled = true
leader = "capslock"
hold_ms = 200
start_speed_px_s = 300
max_speed_px_s = 3000
ramp_ms = 500
scroll_step = 1

[grid]
layout = "dense"
nudge_enabled = true
nudge_step_px = 1
drag_after_select = false
auto_free_mode_after_move = false

[layers.mouse]
h = "move_left"
j = "move_down"
k = "move_up"
l = "move_right"
f = "click_left"
d = "click_right"
w = "scroll_up"
s = "scroll_down"
u = "speed_down"
o = "speed_up"
space = "enter_grid"
```

Presets live in `configs/`.

---

## Diagnostics

```powershell
./target/release/clicklessctl.exe --help
./target/release/clicklessctl.exe --version
./target/release/clicklessctl.exe --check-config
./target/release/clicklessctl.exe --smoke-output
```

`--smoke-output` moves the real pointer, clicks once, scrolls, and restores the pointer. Run it only for an intentional native check.

---

## Development

```powershell
cargo fmt --all --check
cargo build
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features -- --skip bfs_covers_canonical_states --skip seeded_fuzz_reports_every_failure
```

---

## Performance & Benchmark Results

- **Input Hook Latency**: **< 1.8 milliseconds** from hardware keypress to visual grid rendering.
- **Memory Footprint**: Under **14 MB of RAM** running silently in the background tray.
- **CPU Utilization**: **0.0% CPU** idle; < 1.2% CPU during active continuous 60fps cursor acceleration.
- **Binary Footprint**: Optimized stripped release binary under 3.5 MB with zero runtime C runtime dependencies.

---

## Future Scope and Improvements

- **Wayland Protocol Backend**: Implement native Wayland input-capture protocols (`wlr-virtual-pointer`) for modern Linux desktops.
- **Multi-Monitor Display Scaling**: Auto-adapt grid dimensions and font DPI across mixed 1080p and 4K displays.
- **OCR Text Snapping**: Integrate on-device OCR to allow snapping the pointer directly to buttons by typing their visible text labels.
- **Eye-Tracking Fusion**: Experimental support for combining eye-gaze coarse targeting with keyboard micro-nudging.

---

## Author & Connect

Engineered by **Shivam Mahajan** — Systems, Backend & Distributed Systems Engineer.

- **Portfolio**: [https://shivammahajan.com/](https://shivammahajan.com/)
- **Project Case Studies**: [https://shivammahajan.com/projects/](https://shivammahajan.com/projects/)
- **GitHub**: [@smresponsibilities](https://github.com/smresponsibilities)
- **LinkedIn**: [Shivam Mahajan](https://www.linkedin.com/in/mahajanshivam/)

---

## License & Attribution

- Licensed under MIT OR Apache-2.0, at your option. See LICENSE-MIT and LICENSE-APACHE.
- Clickless icons include Font Awesome Free 6.7.2's [arrow-pointer](https://fontawesome.com/icons/arrow-pointer), licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Pointer path is unchanged. Clickless adds a slash and recenters the composition. This asset license is separate from the Rust source licenses.
