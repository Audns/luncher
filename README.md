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

## Installation

### Prerequisites

- Rust toolchain (version 1.70 or newer)
- Wayland development libraries
- XKB common development files

## Cargo install

```bash
  cargo install --git https://github.com/Audns/luncher
```

### Build from Source

```bash
# Clone the repository
git clone <repository-url>
cd luncher

# Build the project
cargo build --release

# The binary will be available at target/release/luncher
```

## Usage

Run the launcher:

```bash
./target/release/luncher
```

Configuration options can be adjusted in the configuration file or through command-line arguments.

## Configuration

Luncher uses a TOML configuration file located at `~/.config/luncher/config.toml` (or XDG_CONFIG_HOME equivalent).

```toml
# The same of your system. This fixes the ugly stretching animation of fractional scaling.
scale = 1.25 
[window]
width = 1200
height = 800
```

### Example

Luncher uses a TOML configuration file located at `~/.config/luncher/scripts.toml` (or XDG_CONFIG_HOME equivalent).

```toml
[reload-waybar]
command = "pkill waybar; sleep 0.2; waybar & disown"

```

## Dependencies

Luncher relies on the following key dependencies:

- **calloop** - Event loop abstraction
- **smithay-client-toolkit** - Wayland client utilities
- **wayland-client & wayland-protocols** - Wayland communication
- **fontdue** - Font rendering
- **nucleo** - Fuzzy matching for search
- **serde** - Configuration serialization
- **dirs** - Directory detection

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
