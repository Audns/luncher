# Luncher

A simple scripts launcher for Linux.

## Overview

Luncher is a lightweight application launcher written in Rust that provides a quick way to launch scripts and applications. It uses Wayland for display and input handling, making it suitable for modern Linux desktop setups.

## Features

- Simple and intuitive interface
- Script launching capabilities
- Wayland-based display
- Configuration file support
- Efficient resource usage
- Fuzzel like app launcher

## Installation

### Prerequisites

- Rust toolchain (version 1.70 or newer)

## Cargo install

```bash
  cargo install --git https://github.com/Audns/luncher
```

## Usage

Run the launcher:

```bash
luncher --help
```

Configuration options can be adjusted in the configuration file or through command-line arguments.

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

### Example

Luncher uses a TOML configuration file located at `~/.config/luncher/scripts.toml` (or XDG_CONFIG_HOME equivalent).

```toml
[scripts]
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
