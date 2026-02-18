# Running litra-autotoggle as a systemd service (Linux)

## Installation

1. Install the `litra-autotoggle` binary to `~./local/bin`:

2. Copy the systemd service file:
   ```bash
   mkdir -p ~/.config/systemd/user
   cp litra-autotoggle.service ~/.config/systemd/user/
   ```

3. Create a configuration file at `~/.config/litra-autotoggle/config.yml`:
   ```bash
   mkdir -p ~/.config/litra-autotoggle
   cp litra-autotoggle.example.yml ~/.config/litra-autotoggle/config.yml
   # Edit as needed
   ```

4. Enable and start the service:
   ```bash
   systemctl --user daemon-reload
   systemctl --user enable litra-autotoggle
   systemctl --user start litra-autotoggle
   ```

## Usage

View logs:
```bash
journalctl --user -u litra-autotoggle -f
```

Stop the service:
```bash
systemctl --user stop litra-autotoggle
```

Restart the service:
```bash
systemctl --user restart litra-autotoggle
```

Check service status:
```bash
systemctl --user status litra-autotoggle
```

## Uninstall

```bash
systemctl --user disable litra-autotoggle
rm ~/.config/systemd/user/litra-autotoggle.service
systemctl --user daemon-reload
```
