# `litra-autotoggle`

💡 Automatically turn your Logitech Litra device on when your webcam turns on, and off when your webcam turns off (macOS and Linux only)

---

## Supported devices

The following Logitech Litra devices, **connected via USB**, are supported:

- [Logitech Litra Glow](https://www.logitech.com/en-gb/products/lighting/litra-glow.946-000002.html)
- [Logitech Litra Beam](https://www.logitech.com/en-gb/products/lighting/litra-beam.946-000007.html)
- [Logitech Litra Beam LX](https://www.logitechg.com/en-gb/products/cameras-lighting/litra-beam-lx-led-light.946-000015.html)

## Installation

### macOS with [Homebrew](https://brew.sh/)

1. Install the latest version of `litra-autotoggle` by running `brew tap timrogers/tap && brew install litra-autotoggle`.
1. Run `litra-autotoggle --help` to check that everything is working.

### All other platforms (using Cargo)

1. Install [Rust](https://www.rust-lang.org/tools/install) on your machine, if it isn't already installed.
1. Install the `litra-autotoggle` crate by running `cargo install litra-autotoggle`.
1. Run `litra-autotoggle --help` to check that everything is working and see the available commands.

### All other platforms (via binary)

1. Download the [latest release](https://github.com/timrogers/litra-autotoggle/releases/latest) for your platform. macOS and Linux devices are supported.
1. Add the binary to `$PATH`, so you can execute it from your shell. For the best experience, call it `litra-autotoggle`.
1. Run `litra-autotoggle --help` to check that everything is working.

## Configuring `udev` permissions on Linux

On most Linux operating systems, you will need to manually configure permissions using [`udev`](https://www.man7.org/linux/man-pages/man7/udev.7.html) to allow non-`root` users to access and manage Litra devices.

To allow all users that are part of the `video` group to access the Litra devices, copy the [`99-litra.rules`](99-litra.rules) file into `/etc/udev/rules.d`.

Next, reboot your computer or run the following commands as `root`:

    # udevadm control --reload-rules
    # udevadm trigger

## Usage

### In the background using Homebrew Services (only when installed with Homebrew on macOS)

Run `brew services start litra-autotoggle`.

`litra-autotoggle` will run in the background, and your Litra will turn on when your webcam turns on, and off when your webcam turns off.

### From the command line

Run `litra-autotoggle`, with an optional `--serial-number` argument. (You can get the serial number using the `litra devices` command in the [`litra`](https://github.com/timrogers/litra-rs) CLI.)

Your Litra will turn on when your webcam turns on, and off when your webcam turns off.
