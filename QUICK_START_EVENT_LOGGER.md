# Quick Start: System Event Logger

A simple logging system that tracks Ubuntu system events to `$HOME/login_logout_times.log`.

## What Gets Logged

- ✅ System startup/boot
- ✅ User login
- ✅ User logout  
- ✅ Screen lock
- ✅ Screen unlock
- ✅ System shutdown

## Installation (One Command)

```bash
cd ~/code/enhanced-terminal-mcp && ./install-event-logger.sh --system-services yes
```

This will:
1. Install logging scripts to `~/.local/bin/`
2. Set up user services (login, logout, screen lock monitoring)
3. Set up system services (boot, shutdown) with sudo prompt
4. Start all services immediately

## Verify Installation

```bash
# Check if services are running
systemctl --user status screen-lock-monitor.service

# View the log file
tail -f ~/login_logout_times.log
```

## View Logs

```bash
# Real-time monitoring
tail -f ~/login_logout_times.log

# Last 50 entries
tail -n 50 ~/login_logout_times.log

# Search for specific events
grep "startup" ~/login_logout_times.log
grep "screen lock" ~/login_logout_times.log
grep "login" ~/login_logout_times.log
```

## Log Format

```
2026-02-05 08:30:45 startup
2026-02-05 14:27:57 login
2026-02-05 14:30:15 screen lock
2026-02-05 14:30:45 screen unlock
2026-02-05 17:45:23 logout
2026-02-05 17:45:28 shutdown
```

## Uninstall

```bash
cd ~/code/enhanced-terminal-mcp && ./uninstall-event-logger.sh
```

## Troubleshooting

**Screen lock not logging?**
```bash
systemctl --user restart screen-lock-monitor.service
journalctl --user -u screen-lock-monitor.service -f
```

**Boot/shutdown not logging?**
```bash
systemctl status system-event-boot.service
systemctl status system-event-shutdown.service
```

**Log file permissions?**
```bash
ls -la ~/login_logout_times.log
chmod 644 ~/login_logout_times.log  # Fix if needed
```

## Files Installed

- `~/.local/bin/system-event-logger.sh` - Main logging script
- `~/.local/bin/screen-lock-monitor.sh` - Screen lock monitor
- `~/.config/systemd/user/system-event-login.service` - Login tracking
- `~/.config/systemd/user/system-event-logout.service` - Logout tracking
- `~/.config/systemd/user/screen-lock-monitor.service` - Lock monitor
- `/etc/systemd/system/system-event-boot.service` - Boot tracking (system)
- `/etc/systemd/system/system-event-shutdown.service` - Shutdown tracking (system)

## More Information

See `EVENT_LOGGER_README.md` for complete documentation.