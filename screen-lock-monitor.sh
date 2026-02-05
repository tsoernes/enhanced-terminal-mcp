#!/bin/bash
# Screen Lock Monitor
# Monitors GNOME screen lock/unlock events and logs them

LOG_SCRIPT="${HOME}/.local/bin/system-event-logger.sh"

# Check if the logging script exists
if [ ! -f "$LOG_SCRIPT" ]; then
    echo "ERROR: Logging script not found at $LOG_SCRIPT" >&2
    exit 1
fi

# Monitor GNOME screensaver lock/unlock events
dbus-monitor --session "type='signal',interface='org.gnome.ScreenSaver'" | \
while read -r line; do
    if echo "$line" | grep -q "boolean true"; then
        # Screen locked
        "$LOG_SCRIPT" lock
    elif echo "$line" | grep -q "boolean false"; then
        # Screen unlocked
        "$LOG_SCRIPT" unlock
    fi
done
