# Luncher

All in one fast daemon-backed launcher + clipboard + ... for Wayland.

## Overview

Luncher is a lightweight Wayland launcher written in Rust. It supports three main modes:

- `script` for custom script entries from your config
- `launcher` for desktop applications discovered from `.desktop` files
- `clipboard` for clipboard history managed by the built-in daemon
- `switcher` for switching Hyprland workspaces from the current window list

The UI is intentionally thin and startup-focused. A background daemon keeps launcher and clipboard state warm so opening `luncher` stays fast.

## Features

- Fast Wayland UI
- Script launcher mode
- Desktop application launcher mode
- Clipboard history mode
- Hyprland workspace switcher mode
- Built-in daemon for warm caches
- TOML-based configuration

## Installation

### Prerequisites

- Rust toolchain (version 1.70 or newer)

## Cargo install

```bash
  cargo install --git https://github.com/Audns/luncher
```

## Usage

Show help:

```bash
luncher --help
```

Open specific modes:

```bash
luncher --daemon
luncher -m script
luncher -m launcher
luncher -m clipboard
luncher -m switcher
```

Configuration options can be adjusted in the configuration file.

### Daemon Mode

Run the background daemon without opening the UI:

```bash
luncher --daemon
```

This is the mode to use for session autostart so clipboard tracking is already active before opening `luncher -m clipboard`.

## Configuration

Luncher uses a TOML configuration file located at `~/.config/luncher/config.toml` (or XDG_CONFIG_HOME equivalent).

```toml
# The same of your system. This fixes the ugly stretching animation of fractional scaling.
scale = 1.25 
single_instance = true
case_sensitive = false
[window]
width = 1200
height = 800

[clipboard]
# Maximum number of clipboard entries kept in memory and shown in the UI.
history_limit = 50

[font]
# Paths to .ttf/.otf/.ttc font files. The first entry that opens successfully
# is used; remaining entries are skipped. If none of the configured paths
# open, the built-in defaults are tried before the renderer panics.
primary = ["/usr/share/fonts/noto/NotoSans-Regular.ttf"]
fallback = ["/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf"]
emoji = ["/usr/share/fonts/noto/NotoColorEmoji.ttf"]
cjk = [
  "/usr/share/fonts/adobe-source-han-sans/SourceHanSansCN-Regular.otf",
  "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
]
```

### Clipboard

The `[clipboard]` section controls the clipboard history mode.

- `history_limit` (`usize`, default `50`) — caps how many entries the daemon
  holds and how many are returned to the UI when `luncher -m clipboard` opens.
  The on-disk `SQLite` history is independently trimmed to a hard cap of `1000`
  entries regardless of this setting, so lowering `history_limit` only affects
  what the UI displays, not what is persisted.

### Fonts

The `[font]` section controls which font files the renderer uses. Each field
is a list of paths; the renderer tries them in order and uses the first one
that opens successfully. If every configured path fails, the built-in defaults
are tried as a fallback. The primary font is required — the renderer will
panic at startup if no candidate can be opened.

- `primary` (`[String]`, default `["/usr/share/fonts/noto/NotoSans-Regular.ttf"]`)
  — the main text font.
- `fallback` (`[String]`, default `["/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf"]`)
  — consulted for glyphs the primary font does not contain.
- `emoji` (`[String]`, default `["/usr/share/fonts/noto/NotoColorEmoji.ttf"]`)
  — color-emoji font, tried between primary and fallback.
- `cjk` (`[String]`, default Source Han Sans CN, then Noto Sans CJK)
  — tried last for CJK code points.

### Scripts Example

Luncher reads script entries from `~/.config/luncher/scripts.toml`.

```toml
[reload_waybar]
name = "reload waybar"
command = "pkill waybar; sleep 0.2; waybar & disown"
tag = ["sys", "waybar"]

```

## Dependencies

See `Cargo.toml` for the complete dependency list.

## Development

To contribute to Luncher:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Submit a pull request

Please ensure your code follows the existing Rust formatting and conventions.

## License

This project is licensed under the MIT License.
