# Goofy OS 
*A Lightweight, Feature-Rich Operating System Built from Scratch in Rust*

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024%20edition-red.svg)](https://www.rust-lang.org/)
[![Architecture](https://img.shields.io/badge/arch-x86__64-green.svg)]()
[![Boot](https://img.shields.io/badge/boot-UEFI%2FBIOS-orange.svg)]()

## Overview

Goofy OS is a lightweight, feature-rich operating system built from scratch in Rust. It aims to provide a modern computing experience while showcasing advanced OS concepts and design principles.

**Preview**
https://github.com/user-attachments/assets/ccc8e8ea-2149-4c18-8c31-aaea638619d7

## Features

### **Desktop Environment**
- **Full GUI Desktop**: Complete windowing system with modern aesthetics
- **Window Manager**: Multi-window support with drag-and-drop functionality
- **Taskbar & Start Menu**: Familiar desktop experience with application launcher
- **Multiple Applications**:
  - **Calculator**: Full-featured calculator with arithmetic operations
  - **Notepad**: Text editor with cursor support and scrolling
  - **System Information**: Real-time system monitoring and statistics

### **Core System Features**
- **Memory Management**:
  - Custom heap allocator with allocation tracking
  - Virtual memory with paging support
  - Frame allocation and memory mapping
  - Memory protection and isolation

- **Interrupt Handling**:
  - Complete Interrupt Descriptor Table (IDT) implementation
  - Hardware interrupt support (keyboard, mouse, timer)
  - Interrupt-driven I/O for responsive user interaction

- **Input/Output Systems**:
  - PS/2 keyboard support with full key mapping
  - PS/2 mouse support with click detection and movement
  - Serial port debugging and logging
  - Framebuffer graphics with custom font rendering

- **Processes and custom ELF loading**:
  - Loads and maps ELF binaries into memory
  - Supports user-space processes with isolated memory
  - Context switching and scheduling
  - System calls for user applications

###### Note: processes are developed in the `processes_better` branch and can be extremely unstable.

### **Graphics & Rendering**
- **Custom Graphics Engine**:
  - Framebuffer-based rendering system
  - Shape primitives (rectangles, text, lines)
  - Surface composition and layering
  - Multiple font sizes and weights (Noto Sans Mono)
  - Color management and transparency support

- **Advanced Text Rendering**:
  - Anti-aliased font rendering
  - Multiple font weights (Light, Regular, Bold)
  - Scalable font sizes (Size16, Size20, Size24, Size28, Size32)
  - Text cursor support with positioning

### **System Architecture**
- **Global Descriptor Table (GDT)**: Proper x86_64 segmentation
- **Real-Time Clock (RTC)**: UTC time tracking and second precision
- **Boot Support**: Both UEFI* and Legacy BIOS compatibility
- **Cross-Platform**: Designed for x86_64 architecture
- **No Standard Library**: Complete `#![no_std]` implementation

### **Development Features**
- **Debug Support**: Serial logging and QEMU integration
- **Panic Handling**: Graceful error handling and debugging information

### Boot Process
1. **Bootloader**: UEFI*/BIOS bootloader loads kernel
2. **Memory Setup**: Physical memory mapping and heap initialization
3. **System Initialization**: GDT, IDT, and interrupt setup
4. **Hardware Detection**: Framebuffer and input device initialization
5. **Desktop Launch**: GUI environment with window manager

###### * UEFI Support is a WIP

## Getting Started

### Prerequisites
- **Rust Toolchain** (2024 edition)
- **QEMU**
- **Nightly Rust**

### Required Rust Components
```bash
# Install Rust nightly
rustup install nightly
rustup default nightly

# Add target for bare-metal
rustup target add x86_64-unknown-none
```

### Building & Running

#### 1. Clone the Repository
```bash
git clone https://github.com/retrogradedev/goofy-os.git
cd goofy-os
```

#### 2. Build the Kernel
```bash
# Build the entire project
cargo build

# Build optimized release version
cargo build --release
```

#### 3. Run in QEMU
```bash
# Run the OS in QEMU emulator
cargo run
```

## Using Goofy OS

### Desktop Interface
- **Mouse**: Click and drag to interact with windows
- **Start Menu**: Click "Start" to launch applications
- **Window Controls**: Drag windows by clicking and holding the title bar
- **Applications**: Access Calculator, Notepad, and System Info from Start menu

### Applications Guide

#### Calculator
- Basic arithmetic operations (+, -, *, /)
- Decimal number support
- Clear and equals functionality
- Visual button interface

#### Notepad
- Multi-line text editing
- Cursor positioning with arrow keys
- Scrolling support for long documents
- Real-time text rendering

#### System Information
- Real-time memory usage statistics
- CPU information and system details
- Heap and stack usage monitoring

## Development

### Project Structure
- **Memory Safety**: All code written in safe Rust with minimal unsafe blocks
- **Error Handling**: Comprehensive panic and error handling

## Roadmap & Future Plans

### Current Goals (In Progress)
- [ ] **Window Focus Management**: Implement proper window focusing system
- [ ] **Taskbar Window List**: Show running applications in taskbar
- [ ] **Notepad Improvements**: Fix text editing bugs and enhance functionality
- [ ] **Window Drag Optimizations**: Improve cursor restoration during window dragging

### Short-term Goals (Next 3 months)
- [ ] **File System**: Basic FAT32 file system support
- [ ] **Process Management**: Improve our process scheduling and management
- [ ] **Network Stack**: Basic TCP/IP implementation
- [ ] **More Applications**: Image viewer, terminal emulator

### Long-term Vision (6+ months)
- [ ] **Multi-core Support**: SMP (Symmetric Multi-Processing)
- [ ] **USB Support**: USB device drivers and hot-plug detection
- [ ] **Advanced Graphics**: Hardware-accelerated graphics (GPU support)
- [ ] **Package Manager**: Application installation and management system
- [ ] **Development Tools**: On-system compiler and development environment

### Research Areas
- [ ] **Microkernel Architecture**: Explore microkernel design patterns
- [ ] **Security Features**: Memory protection and sandboxing
- [ ] **Real-time Capabilities**: Real-time scheduling and guarantees
- [ ] **Container Support**: Lightweight containerization

## Contributing

We welcome contributions from developers of all skill levels! Here's how you can help:

### Ways to Contribute
1. ** Bug Reports**: Found a bug? Open an issue with detailed reproduction steps
2. ** Feature Requests**: Have an idea? We'd love to hear about it
3. ** Documentation**: Improve README, comments, or add tutorials
4. ** Code Contributions**: Fix bugs, implement features, or optimize performance
5. ** Testing**: Help test on different hardware configurations

### Development Setup
1. Fork the repository
2. Create a feature branch (`git checkout -b amazing-feature`)
3. Make your changes and test thoroughly
4. Commit with descriptive messages
5. Push to your fork and create a Pull Request

### Coding Standards
- Follow Rust best practices and idioms
- Use `cargo fmt` for consistent formatting
- Add tests for new functionality
- Update documentation for API changes
- Keep commits atomic and well-described

## System Requirements

### Host System (Development)
- **OS**: Windows, Linux, or macOS
- **Rust**: 1.75+ with nightly toolchain
- **Memory**: 4GB RAM minimum, 8GB recommended
- **Storage**: 2GB free space for build artifacts
- **Virtualization**: QEMU 6.0+

### Target System (Runtime)
- **Architecture**: x86_64 (64-bit Intel/AMD)
- **Memory**: 256MB RAM minimum, 512MB recommended
- **Boot**: UEFI or Legacy BIOS support
- **Graphics**: VGA-compatible framebuffer
- **Input**: PS/2 keyboard and mouse

### Tested Platforms
- **QEMU**: Full support with hardware acceleration
- **Physical Hardware**: Untested testing (community feedback welcome)

## Learning Resources

### For Beginners
- [OSDev Wiki](https://wiki.osdev.org/) - Comprehensive OS development resource
- [Rust Book](https://doc.rust-lang.org/book/) - Learn Rust programming language
- [Writing an OS in Rust](https://os.phil-opp.com/) - Excellent tutorial series

### For Advanced Developers  
- [UEFI Specification](https://uefi.org/specifications)
- [x86_64 crate documentation](https://docs.rs/x86_64/)

## Troubleshooting

### Common Issues

#### Build Errors
```bash
# Ensure nightly Rust is installed
rustup install nightly
rustup default nightly

# Add required target
rustup target add x86_64-unknown-none

# Clean build artifacts
cargo clean
```

#### QEMU Issues
```bash
# Install QEMU on Ubuntu/Debian
sudo apt install qemu-system-x86

# Install QEMU on macOS
brew install qemu

# Install QEMU on Windows
# Download from: https://www.qemu.org/download/#windows
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- **Philipp Oppermann**: For the excellent "Writing an OS in Rust" blog series
- **Rust Community**: For creating an amazing language for systems programming  
- **OSDev Community**: For comprehensive documentation and support
- **Bootloader Crate**: For simplifying the boot process
- **QEMU Project**: For providing excellent emulation capabilities

---

<div align="center">

**Made with ❤️ and Rust 🦀**

*Goofy OS - Where learning operating systems is anything but goofy!*

</div>
