#!/bin/bash
# Installation script for system event logger
# This script sets up logging for startups, logins, logouts, screen locks, and shutdowns
#
# Usage: ./install-event-logger.sh [--system-services yes|no]

set -e

# Default values
INSTALL_SYSTEM_SERVICES="ask"

# Parse command-line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --system-services)
            if [[ "$2" == "yes" ]]; then
                INSTALL_SYSTEM_SERVICES="yes"
            elif [[ "$2" == "no" ]]; then
                INSTALL_SYSTEM_SERVICES="no"
            else
                echo "Invalid value for --system-services: $2 (use 'yes' or 'no')"
                exit 1
            fi
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [--system-services yes|no]"
            echo ""
            echo "Options:"
            echo "  --system-services yes|no   Install system-level boot/shutdown tracking"
            echo "                             If not specified, will prompt interactively"
            echo "  -h, --help                Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
USER_BIN_DIR="${HOME}/.local/bin"
USER_SYSTEMD_DIR="${HOME}/.config/systemd/user"
SYSTEM_SYSTEMD_DIR="/etc/systemd/system"
LOG_FILE="${HOME}/login_logout_times.log"

echo "=== System Event Logger Installation ==="
echo ""

# Create necessary directories
echo "Creating directories..."
mkdir -p "$USER_BIN_DIR"
mkdir -p "$USER_SYSTEMD_DIR"

# Install scripts
echo "Installing logging scripts..."
cp "${SCRIPT_DIR}/system-event-logger.sh" "$USER_BIN_DIR/"
cp "${SCRIPT_DIR}/screen-lock-monitor.sh" "$USER_BIN_DIR/"
chmod +x "$USER_BIN_DIR/system-event-logger.sh"
chmod +x "$USER_BIN_DIR/screen-lock-monitor.sh"

# Install user systemd services
echo "Installing user systemd services..."
cp "${SCRIPT_DIR}/system-event-login.service" "$USER_SYSTEMD_DIR/"
cp "${SCRIPT_DIR}/system-event-logout.service" "$USER_SYSTEMD_DIR/"
cp "${SCRIPT_DIR}/screen-lock-monitor.service" "$USER_SYSTEMD_DIR/"

# Reload user systemd daemon
echo "Reloading user systemd daemon..."
systemctl --user daemon-reload

# Enable and start user services
echo "Enabling user services..."
systemctl --user enable system-event-login.service
systemctl --user enable system-event-logout.service
systemctl --user enable screen-lock-monitor.service

echo "Starting user services..."
systemctl --user start system-event-login.service
systemctl --user start screen-lock-monitor.service

# Create log file with initial entry
echo "Creating log file..."
touch "$LOG_FILE"
echo "[$(date '+%Y-%m-%d %H:%M:%S')] SYSTEM EVENT LOGGER INSTALLED - User: $(whoami)" >> "$LOG_FILE"

# Install system services (requires sudo)
echo ""
echo "=== System-level services (boot/shutdown tracking) ==="
echo "These require sudo privileges to install."
echo ""

# Determine whether to install system services
if [[ "$INSTALL_SYSTEM_SERVICES" == "ask" ]]; then
    read -p "Do you want to install system-level boot/shutdown tracking? (y/n) " -n 1 -r
    echo ""
    DO_INSTALL="$REPLY"
elif [[ "$INSTALL_SYSTEM_SERVICES" == "yes" ]]; then
    echo "Installing system services (--system-services yes)"
    DO_INSTALL="y"
else
    echo "Skipping system services (--system-services no)"
    DO_INSTALL="n"
fi

if [[ $DO_INSTALL =~ ^[Yy]$ ]]; then
    # Create temporary askpass script for GUI password prompt
    ASKPASS_SCRIPT="/tmp/askpass-$$.sh"
    cat > "$ASKPASS_SCRIPT" << 'EOF'
#!/bin/bash
zenity --password --title="sudo password for system event logger installation"
EOF
    chmod +x "$ASKPASS_SCRIPT"

    echo "Installing system services..."

    # Use askpass for GUI password prompt
    export SUDO_ASKPASS="$ASKPASS_SCRIPT"

    # Copy system service files
    sudo -A cp "${SCRIPT_DIR}/system-event-boot.service" "$SYSTEM_SYSTEMD_DIR/"
    sudo -A cp "${SCRIPT_DIR}/system-event-shutdown.service" "$SYSTEM_SYSTEMD_DIR/"

    # Fix the %u placeholder with actual username
    sudo -A sed -i "s/%u/$(whoami)/g" "$SYSTEM_SYSTEMD_DIR/system-event-boot.service"
    sudo -A sed -i "s/%u/$(whoami)/g" "$SYSTEM_SYSTEMD_DIR/system-event-shutdown.service"

    # Reload system daemon
    sudo -A systemctl daemon-reload

    # Enable system services
    sudo -A systemctl enable system-event-boot.service
    sudo -A systemctl enable system-event-shutdown.service

    # Clean up askpass script
    rm -f "$ASKPASS_SCRIPT"

    echo "System services installed successfully!"
else
    echo "Skipping system-level services."
fi

echo ""
echo "=== Installation Complete ==="
echo ""
echo "Log file location: $LOG_FILE"
echo ""
echo "Services status:"
echo "  User services:"
systemctl --user status system-event-login.service --no-pager -l || true
systemctl --user status system-event-logout.service --no-pager -l || true
systemctl --user status screen-lock-monitor.service --no-pager -l || true

if [[ $DO_INSTALL =~ ^[Yy]$ ]]; then
    echo ""
    echo "  System services:"
    systemctl status system-event-boot.service --no-pager -l || true
    systemctl status system-event-shutdown.service --no-pager -l || true
fi

echo ""
echo "To view logs: tail -f $LOG_FILE"
echo "To uninstall: run ./uninstall-event-logger.sh"
echo ""
