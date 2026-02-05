#!/bin/bash
# System Event Logger
# Logs system events (startup, login, logout, lock, shutdown) to a file

LOG_FILE="${HOME}/login_logout_times.log"
EVENT_TYPE="$1"

# Create log file if it doesn't exist
touch "$LOG_FILE" 2>/dev/null || {
    echo "ERROR: Cannot create log file at $LOG_FILE" >&2
    exit 1
}

# Get current timestamp
TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')

# Log the event
case "$EVENT_TYPE" in
    boot|startup)
        echo "$TIMESTAMP startup" >> "$LOG_FILE"
        ;;
    login)
        echo "$TIMESTAMP login" >> "$LOG_FILE"
        ;;
    logout)
        echo "$TIMESTAMP logout" >> "$LOG_FILE"
        ;;
    lock)
        echo "$TIMESTAMP screen lock" >> "$LOG_FILE"
        ;;
    unlock)
        echo "$TIMESTAMP screen unlock" >> "$LOG_FILE"
        ;;
    shutdown)
        echo "$TIMESTAMP shutdown" >> "$LOG_FILE"
        ;;
    *)
        echo "$TIMESTAMP unknown: $EVENT_TYPE" >> "$LOG_FILE"
        ;;
esac

exit 0
