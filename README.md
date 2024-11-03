# `litra-autotoggle`

💡 Automatically turn your Logitech Litra device on when your webcam turns on, and off when your webcam turns off (macOS and Linux only)

---

## Supported devices

The following Logitech Litra devices, **connected via USB**, are supported:

- [Logitech Litra Glow](https://www.logitech.com/en-gb/products/lighting/litra-glow.946-000002.html)
- [Logitech Litra Beam](https://www.logitech.com/en-gb/products/lighting/litra-beam.946-000007.html)
- [Logitech Litra Beam LX](https://www.logitechg.com/en-gb/products/cameras-lighting/litra-beam-lx-led-light.946-000015.html)

## Installation

### macOS or Linux via [Homebrew](https://brew.sh/)

1. Install the latest version of `litra-autotoggle` by running `brew tap timrogers/tap && brew install litra-autotoggle`.
1. Run `litra-autotoggle --help` to check that everything is working.

### macOS or Linux via [Cargo](https://doc.rust-lang.org/cargo/), Rust's package manager

1. Install [Rust](https://www.rust-lang.org/tools/install) on your machine, if it isn't already installed.
1. Install the `litra-autotoggle` crate by running `cargo install litra-autotoggle`.
1. Run `litra-autotoggle --help` to check that everything is working and see the available commands.

### macOS or Linux via direct binary download

1. Download the [latest release](https://github.com/timrogers/litra-autotoggle/releases/latest) for your platform. macOS and Linux devices are supported.
1. Add the binary to `$PATH`, so you can execute it from your shell. For the best experience, call it `litra-autotoggle`.
1. Run `litra-autotoggle --help` to check that everything is working.

## Usage

### In the background, using Homebrew Services ([Homebrew](https://brew.sh/) installations only)

Run `brew services start timrogers/tap/litra-autotoggle`.

`litra-autotoggle` will run in the background, and your Litra will turn on when your webcam turns on, and off when your webcam turns off. If no Litra device is connected, the listener will keep on running, but will do nothing.

> [!NOTE]
> When starting the service for the first time on a macOS device, you will receive a notification warning you about software running in the background.

![macOS warning](https://github.com/user-attachments/assets/7abd6d99-0481-4684-8079-a6d80e0fcaea)

### From the command line

Just run `litra-autotoggle`. Your Litra will turn on when your webcam turns on, and off when your webcam turns off.

The following arguments are supported:

- `--serial-number` to point to a specific Litra device. You can get the serial number using the `litra devices` command in the [`litra`](https://github.com/timrogers/litra-rs) CLI.
- `--require-device` to enforce that a Litra device must be connected. By default, the listener will keep running even if no Litra device is found.
- `--video-device` (Linux only) to watch a specific video device (e.g. `/dev/video0`). By default, all video devices will be watched.

## Configuring `udev` permissions (Linux only)

On most Linux operating systems, you will need to manually configure permissions using [`udev`](https://www.man7.org/linux/man-pages/man7/udev.7.html) to allow non-`root` users to access and manage Litra devices.

To allow all users that are part of the `video` group to access the Litra devices, copy the [`99-litra.rules`](99-litra.rules) file into `/etc/udev/rules.d`.

Next, reboot your computer or run the following commands as `root`:

    # udevadm control --reload-rules
    # udevadm trigger
