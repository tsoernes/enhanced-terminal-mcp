#!/bin/bash
# Uninstallation script for system event logger
# This script removes all installed services and scripts

set -e

USER_BIN_DIR="${HOME}/.local/bin"
USER_SYSTEMD_DIR="${HOME}/.config/systemd/user"
SYSTEM_SYSTEMD_DIR="/etc/systemd/system"
LOG_FILE="${HOME}/login_logout_times.log"

echo "=== System Event Logger Uninstallation ==="
echo ""

# Stop and disable user services
echo "Stopping user services..."
systemctl --user stop screen-lock-monitor.service 2>/dev/null || true
systemctl --user stop system-event-login.service 2>/dev/null || true
systemctl --user stop system-event-logout.service 2>/dev/null || true

echo "Disabling user services..."
systemctl --user disable screen-lock-monitor.service 2>/dev/null || true
systemctl --user disable system-event-login.service 2>/dev/null || true
systemctl --user disable system-event-logout.service 2>/dev/null || true

# Remove user service files
echo "Removing user service files..."
rm -f "$USER_SYSTEMD_DIR/system-event-login.service"
rm -f "$USER_SYSTEMD_DIR/system-event-logout.service"
rm -f "$USER_SYSTEMD_DIR/screen-lock-monitor.service"

# Remove scripts
echo "Removing scripts..."
rm -f "$USER_BIN_DIR/system-event-logger.sh"
rm -f "$USER_BIN_DIR/screen-lock-monitor.sh"

# Reload user systemd daemon
echo "Reloading user systemd daemon..."
systemctl --user daemon-reload

# Check if system services exist
SYSTEM_SERVICES_EXIST=false
if [ -f "$SYSTEM_SYSTEMD_DIR/system-event-boot.service" ] || [ -f "$SYSTEM_SYSTEMD_DIR/system-event-shutdown.service" ]; then
    SYSTEM_SERVICES_EXIST=true
fi

if [ "$SYSTEM_SERVICES_EXIST" = true ]; then
    echo ""
    echo "=== System-level services removal ==="
    echo "System services were found. These require sudo to remove."
    echo ""
    read -p "Do you want to remove system-level boot/shutdown tracking? (y/n) " -n 1 -r
    echo ""

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        # Create temporary askpass script for GUI password prompt
        ASKPASS_SCRIPT="/tmp/askpass-$$.sh"
        cat > "$ASKPASS_SCRIPT" << 'EOF'
#!/bin/bash
zenity --password --title="sudo password for system event logger uninstallation"
EOF
        chmod +x "$ASKPASS_SCRIPT"

        echo "Removing system services..."

        # Use askpass for GUI password prompt
        export SUDO_ASKPASS="$ASKPASS_SCRIPT"

        # Stop and disable system services
        sudo -A systemctl stop system-event-boot.service 2>/dev/null || true
        sudo -A systemctl stop system-event-shutdown.service 2>/dev/null || true
        sudo -A systemctl disable system-event-boot.service 2>/dev/null || true
        sudo -A systemctl disable system-event-shutdown.service 2>/dev/null || true

        # Remove system service files
        sudo -A rm -f "$SYSTEM_SYSTEMD_DIR/system-event-boot.service"
        sudo -A rm -f "$SYSTEM_SYSTEMD_DIR/system-event-shutdown.service"

        # Reload system daemon
        sudo -A systemctl daemon-reload

        # Clean up askpass script
        rm -f "$ASKPASS_SCRIPT"

        echo "System services removed successfully!"
    else
        echo "Skipping system services removal."
    fi
fi

echo ""
echo "=== Uninstallation Complete ==="
echo ""
echo "Log file preserved at: $LOG_FILE"
echo ""
read -p "Do you want to delete the log file as well? (y/n) " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    rm -f "$LOG_FILE"
    echo "Log file deleted."
else
    echo "Log file preserved."
fi

echo ""
echo "System event logger has been uninstalled."
echo ""
