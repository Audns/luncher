# Luncher

A fast daemon-backed launcher for Wayland.

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
```

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
