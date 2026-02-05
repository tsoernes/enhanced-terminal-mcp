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

# Get additional context
HOSTNAME=$(hostname)
USER=$(whoami)

# Log the event
case "$EVENT_TYPE" in
    boot|startup)
        echo "[$TIMESTAMP] SYSTEM BOOT - Host: $HOSTNAME, User: $USER" >> "$LOG_FILE"
        ;;
    login)
        echo "[$TIMESTAMP] USER LOGIN - User: $USER, Display: ${DISPLAY:-N/A}, Session: ${XDG_SESSION_ID:-N/A}" >> "$LOG_FILE"
        ;;
    logout)
        echo "[$TIMESTAMP] USER LOGOUT - User: $USER, Session: ${XDG_SESSION_ID:-N/A}" >> "$LOG_FILE"
        ;;
    lock)
        echo "[$TIMESTAMP] SCREEN LOCKED - User: $USER" >> "$LOG_FILE"
        ;;
    unlock)
        echo "[$TIMESTAMP] SCREEN UNLOCKED - User: $USER" >> "$LOG_FILE"
        ;;
    shutdown)
        echo "[$TIMESTAMP] SYSTEM SHUTDOWN - Host: $HOSTNAME, User: $USER" >> "$LOG_FILE"
        ;;
    *)
        echo "[$TIMESTAMP] UNKNOWN EVENT: $EVENT_TYPE - User: $USER" >> "$LOG_FILE"
        ;;
esac

exit 0
