# `litra-autotoggle`

💡 Automatically turn your Logitech Litra device on when your webcam turns on, and off when your webcam turns off (macOS, Linux, and Windows)

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

### macOS, Linux, or Windows via [Cargo](https://doc.rust-lang.org/cargo/), Rust's package manager

1. Install [Rust](https://www.rust-lang.org/tools/install) on your machine, if it isn't already installed.
1. Install the `litra-autotoggle` crate by running `cargo install litra-autotoggle`.
1. Run `litra-autotoggle --help` to check that everything is working and see the available commands.

### macOS, Linux, or Windows via direct binary download

1. Download the [latest release](https://github.com/timrogers/litra-autotoggle/releases/latest) for your platform. macOS, Linux, and Windows devices are supported.
1. Add the binary to your `PATH` (or `$PATH` on Unix-like systems), so you can execute it from your shell/terminal. For the best experience, call it `litra-autotoggle` (or `litra-autotoggle.exe` on Windows).
1. Run `litra-autotoggle --help` to check that everything is working.

## Usage

### In the background, using Homebrew Services ([Homebrew](https://brew.sh/) installations only)

Run `brew services start timrogers/tap/litra-autotoggle`.

`litra-autotoggle` will run in the background, and all connected Litra devices will turn on when your webcam turns on, and off when your webcam turns off. If no Litra device is connected, the listener will keep on running, but will do nothing.

To customize the background service's configuration, edit the config file at `$(brew --prefix)/etc/litra-autotoggle.yml`. For information on how `litra-autotoggle` config files work, see "Using a configuration file" below. To validate your config file, run `litra-autotoggle --config-file $(brew --prefix)/etc/litra-autotoggle.yml`.

> [!NOTE]
> When starting the service for the first time on a macOS device, you will receive a notification warning you about software running in the background.

![macOS warning](https://github.com/user-attachments/assets/7abd6d99-0481-4684-8079-a6d80e0fcaea)

### From the command line

Just run `litra-autotoggle`. By default, all connected Litra devices will turn on when your webcam turns on, and off when your webcam turns off.

The following arguments are supported:

- `--config-file` to specify a YAML configuration file containing options (see "Using a configuration file" below)
- `--serial-number` to point to a specific Litra device by serial number. You can get the serial number using the `litra devices` command in the [`litra`](https://github.com/timrogers/litra-rs) CLI.
- `--device-path` to point to a specific Litra device by its path (useful for devices that don't show a serial number).
- `--device-type` to point to a specific Litra device type (`glow`, `beam` or `beam_lx`).
- `--require-device` to enforce that a Litra device must be connected. By default, the listener will keep running even if no Litra device is found. With this set, the listener will exit whenever it looks for a Litra device and none is found.
- `--video-device` (Linux only) to watch a specific video device (e.g. `/dev/video0`). By default, all video devices will be watched.
- `--delay` to customize the delay (in milliseconds) between a webcam event being detected and toggling your Litra. When your webcam turns on or off, multiple events may be generated in quick succession. Setting a delay allows the program to wait for all events before taking action, avoiding flickering. Defaults to 1.5 seconds (1500 milliseconds).
- `--verbose` to enable verbose logging
- `--back` to toggle the back light on Litra Beam LX devices. When enabled, the back light will be turned on/off together with the front light.

> [!NOTE]
> Only one filter (`--serial-number`, `--device-path`, or `--device-type`) can be specified at a time.

### Using a configuration file

Instead of passing arguments on the command line, you can use a YAML configuration file with the `--config-file` option. This is particularly useful when running `litra-autotoggle` as a background service.

Create a YAML file (e.g., `config.yml`) with your desired options:

```yaml
# By default, the tool will control all connected Litra devices. You can specify ONE
# of the below filters to limit which device(s) it will control.
#
# device_type: glow
# serial_number: ABCD1
# device_path: DevSrvsID:4296789687
#
# By default, the tool will watch all connected video devices. On Linux, you can limit
# this to one specific device by specifying its path below.
#
# video_device: /dev/video0
#
# By default, the tool will wait 1.5 seconds after a video device event before toggling
# the light to reduce flickering. You can adjust this delay (in milliseconds) below.
#
# delay: 2000
#
# By default, if no Litra devices are found, the tool will keep running. You can change this
# behavior by setting the option below to true.
#
# require_device: true
#
# By default, the tool emits simple logs. You can enable debug logging by setting the option
# below to true.
#
# verbose: true
#
# By default, only the front light is toggled. On Litra Beam LX devices, you can also toggle
# the back light by setting the option below to true.
#
# back: true
```

Then run:

```bash
litra-autotoggle --config-file config.yml
```

**Available configuration options:**

All command-line options can be specified in the configuration file using underscored names:

- `serial_number` (string)
- `device_path` (string)
- `device_type` (string: `glow`, `beam`, or `beam_lx`)
- `require_device` (boolean)
- `video_device` (string, Linux only)
- `delay` (number, in milliseconds)
- `verbose` (boolean)
- `back` (boolean, toggles the back light on Litra Beam LX devices)

**Important notes:**

- Command-line arguments take precedence over configuration file values
- The configuration file is strictly validated - unknown fields or invalid values will cause an error
- Only one filter (`serial_number`, `device_path`, or `device_type`) can be specified in the config file
- See [`litra-autotoggle.example.yml`](litra-autotoggle.example.yml) for a complete example with all available options

## Configuring `udev` permissions (Linux only)

On most Linux operating systems, you will need to manually configure permissions using [`udev`](https://www.man7.org/linux/man-pages/man7/udev.7.html) to allow non-`root` users to access and manage Litra devices.

To allow all users that are part of the `video` group to access the Litra devices, copy the [`99-litra.rules`](99-litra.rules) file into `/etc/udev/rules.d`.

Next, reboot your computer or run the following commands as `root`:

    # udevadm control --reload-rules
    # udevadm trigger

## Windows-specific notes

On Windows, `litra-autotoggle` monitors camera usage by polling the Windows registry. The application checks the registry path:

```
HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\webcam
```

The registry contains entries for each application that has accessed the camera, with `LastUsedTimeStart` and `LastUsedTimeStop` timestamps. The camera is considered active when any application has a `LastUsedTimeStart` timestamp greater than its `LastUsedTimeStop` timestamp.

The application polls the registry every 500ms for changes. This approach is compatible with all Windows applications that use the standard camera APIs.
