# amdgpu-top-tui2

An alternate [nvitop](https://github.com/XuehaiPan/nvitop)/[btop](https://github.com/aristocratos/btop)-style
terminal UI for AMD systems — CPU, AMD GPUs (discrete *and* APUs), and the
Strix Halo XDNA NPU — built on top of
[`amdgpu_top`](https://github.com/Umio-Yasuno/amdgpu_top)'s `libamdgpu_top`
backend.

> **This is an add-on for [`amdgpu_top`](https://github.com/Umio-Yasuno/amdgpu_top), not a standalone tool.**
> All telemetry comes from `libamdgpu_top` (the same library that powers
> `amdgpu_top`'s `--tui`, `--gui`, and `--json` frontends). Think of this as a
> fourth, alternate TUI frontend — hence `tui2` — focused on the nvitop/btop
> aesthetic. Huge credit to [@Umio-Yasuno](https://github.com/Umio-Yasuno) for
> the backend that does all the hard work.

## Why

`nvitop` is great but NVIDIA-only. On AMD — especially a Strix Halo box with an
APU **and** an XDNA NPU, or a workstation with multiple cards (e.g. 7900 XTX +
W7900) — there was no single TUI with that look-and-feel. This fills that gap by
rendering `libamdgpu_top`'s data in a modern, themeable layout.

## Features

- **Collapsible CPU / GPU / NPU / Processes sections** (state persists across runs)
- **CPU section, btop-style**: per-core grid with braille history mini-graphs,
  package temp/power, load average, plus system MEM/SWP
- **Multi-GPU**: one band per device, labeled by index + PCI bus-id
- **APU-aware memory**: shows the real GTT (unified system RAM) pool on APUs,
  VRAM on discrete cards — both labeled `MEM`
- **XDNA NPU**: utilization + per-context table (when present)
- **Process table**: per-process VRAM/GTT and engine usage via `fdinfo`
- **All [btop](https://github.com/aristocratos/btop) themes supported** — drops
  into the same `.theme` files; cycle live with `t`/`T`. Defaults to `onedark`.
- nvitop-style fixed-track gauges with aligned numeric columns; braille area
  graphs with theme-gradient fills

## Install

### 1. From source (works today)

Requires a **Rust toolchain** and **`libdrm` development headers**, plus an AMD
GPU/APU running the `amdgpu` kernel driver.

```sh
cargo install --git https://github.com/lhl/amdgpu_top_tui2
```

This installs the `amdgpu-top-tui2` binary. The `libamdgpu_top` backend is
pulled in and compiled automatically (statically linked) — you do **not** need
the `amdgpu_top` binary installed separately. The only runtime dependency is
`libdrm_amdgpu.so.1`, which is present on any system with AMD drivers.

Distro packages for the `libdrm` build headers:

| Distro | Package |
|---|---|
| Arch | `libdrm` |
| Debian/Ubuntu | `libdrm-dev` |
| Fedora | `libdrm-devel` |

> **Note:** `cargo install` from crates.io is not currently possible because we
> depend on `libamdgpu_top` via git (it is not published to crates.io). Prebuilt
> release binaries / AUR packaging are planned.

## Usage

```sh
amdgpu-top-tui2
```

### Keybindings

| Key | Action |
|---|---|
| `q` / `Esc` | quit |
| `Tab` / `Shift+Tab` | move between sections |
| `Space` / `Enter` | collapse / expand the focused section |
| `t` / `T` | next / previous theme |
| `b` / `B` | next / previous gauge block style |

Section collapse state, the selected theme, and the gauge block style all
persist across runs.

### Gauge block styles

Cycle the bar fill glyph with `b`/`B`:
`3/4` (▊, default), `smooth` (precise fractional █), `dotmatrix` (⣿ LED cell),
`lines` (━/─), `squares` (■/□), `rects` (▮/▯), `pills` (▰/▱).

## Themes

Themes are read from the standard btop locations (first match wins):

```
$XDG_CONFIG_HOME/btop/themes/
~/.config/btop/themes/
/usr/local/share/btop/themes/
/usr/share/btop/themes/
```

Any btop `.theme` file works (hex `#RRGGBB`, 2-char grayscale `#BW`, and
`R G B` decimal color formats are all supported, including gradients). The
default is `onedark`; a minimal everforest fallback is bundled in case no theme
files are installed.

## Credits

- Backend: [`libamdgpu_top`](https://github.com/Umio-Yasuno/amdgpu_top) by Umio-Yasuno
- Inspiration: [nvitop](https://github.com/XuehaiPan/nvitop), [btop](https://github.com/aristocratos/btop)

## License

MIT
