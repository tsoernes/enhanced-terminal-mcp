# System Event Logger for Ubuntu

A comprehensive logging system that tracks system startups, user logins/logouts, screen locks/unlocks, and shutdowns on Ubuntu with GNOME.

## Features

- **System Boot Tracking**: Logs when the system starts up
- **User Login/Logout Tracking**: Records when users log in and out of their sessions
- **Screen Lock/Unlock Monitoring**: Tracks when the screen is locked and unlocked
- **System Shutdown Tracking**: Logs when the system is shutting down
- **Timestamped Entries**: All events include precise timestamps
- **User Context**: Logs include username, hostname, display, and session information
- **Persistent Logging**: Uses systemd services for reliable event capture

## Log File Location

All events are logged to: `$HOME/login_logout_times.log`

## Installation

1. Make the installation script executable:
```bash
chmod +x install-event-logger.sh
```

2. Run the installation script:
```bash
./install-event-logger.sh
```

3. The script will:
   - Install logging scripts to `~/.local/bin/`
   - Install user systemd services for login/logout/screen lock tracking
   - Optionally install system services for boot/shutdown tracking (requires sudo)

4. When prompted for system services, choose 'y' to enable boot and shutdown tracking

## What Gets Installed

### User-level Services (No sudo required)
- `system-event-login.service` - Logs when you log in
- `system-event-logout.service` - Logs when you log out
- `screen-lock-monitor.service` - Monitors and logs screen lock/unlock events

### System-level Services (Requires sudo)
- `system-event-boot.service` - Logs system boot events
- `system-event-shutdown.service` - Logs system shutdown events

### Scripts
- `~/.local/bin/system-event-logger.sh` - Main logging script
- `~/.local/bin/screen-lock-monitor.sh` - Screen lock monitoring script

## Log Format

Each log entry follows this format:
```
YYYY-MM-DD HH:MM:SS event_type
```

Event types: `startup`, `login`, `screen lock`, `screen unlock`, `logout`, `shutdown`

Example entries:
```
2024-01-15 08:30:45 startup
2024-01-15 08:31:12 login
2024-01-15 09:15:30 screen lock
2024-01-15 09:16:02 screen unlock
2024-01-15 17:45:23 logout
2024-01-15 17:45:28 shutdown
```

## Viewing Logs

View the log file in real-time:
```bash
tail -f ~/login_logout_times.log
```

View all logs:
```bash
cat ~/login_logout_times.log
```

View recent logs:
```bash
tail -n 50 ~/login_logout_times.log
```

## Managing Services

### Check service status

User services:
```bash
systemctl --user status system-event-login.service
systemctl --user status system-event-logout.service
systemctl --user status screen-lock-monitor.service
```

System services:
```bash
systemctl status system-event-boot.service
systemctl status system-event-shutdown.service
```

### Start/Stop services manually

```bash
systemctl --user start screen-lock-monitor.service
systemctl --user stop screen-lock-monitor.service
```

### View service logs

```bash
journalctl --user -u screen-lock-monitor.service -f
```

## Uninstallation

1. Make the uninstall script executable:
```bash
chmod +x uninstall-event-logger.sh
```

2. Run the uninstall script:
```bash
./uninstall-event-logger.sh
```

3. The script will:
   - Stop and disable all services
   - Remove all installed scripts and service files
   - Optionally remove system services (requires sudo)
   - Optionally delete the log file

## Troubleshooting

### Screen lock monitoring not working

Check if the service is running:
```bash
systemctl --user status screen-lock-monitor.service
```

Restart the service:
```bash
systemctl --user restart screen-lock-monitor.service
```

Check service logs:
```bash
journalctl --user -u screen-lock-monitor.service -n 50
```

### Boot/shutdown events not logging

Verify system services are enabled:
```bash
systemctl status system-event-boot.service
systemctl status system-event-shutdown.service
```

Check if the log file is writable:
```bash
ls -l ~/login_logout_times.log
```

### Log file not being created

Ensure the home directory is writable:
```bash
touch ~/test-write && rm ~/test-write
```

Manually run the logging script:
```bash
~/.local/bin/system-event-logger.sh login
```

## Technical Details

### How It Works

1. **User Login**: Triggered by systemd user service when graphical session starts
2. **User Logout**: Triggered by systemd user service before shutdown
3. **Screen Lock/Unlock**: Monitored via dbus-monitor watching GNOME ScreenSaver signals
4. **System Boot**: Triggered by systemd system service after multi-user target
5. **System Shutdown**: Triggered by systemd system service before shutdown/reboot/halt

### Dependencies

- `systemd` - Service management
- `dbus-monitor` - Screen lock monitoring
- `zenity` - GUI password prompts for sudo (when installing system services)
- GNOME Desktop Environment - For screen lock detection

### Security Considerations

- Log file is created in user's home directory with default permissions
- User services run with user privileges
- System services require sudo only during installation
- No sensitive information is logged (only timestamps and event types)

## Customization

### Changing Log Format

Edit `~/.local/bin/system-event-logger.sh` and modify the echo statements in the case block.

### Adding Custom Events

1. Add a new case in `system-event-logger.sh`
2. Call the script with your custom event type:
```bash
~/.local/bin/system-event-logger.sh my-custom-event
```

### Changing Log Location

1. Edit `system-event-logger.sh` and change the `LOG_FILE` variable
2. Update system service files to use the new path
3. Reload systemd and restart services

## License

This is free and unencumbered software released into the public domain.

## Support

For issues or questions, check:
- Service logs: `journalctl --user -u <service-name>`
- System logs: `journalctl -xe`
- Log file: `~/login_logout_times.log`
