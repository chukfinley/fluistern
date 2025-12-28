#!/bin/bash
# Flüstern Installation Script
# Voice dictation for Linux using Groq Whisper API

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "=== Flüstern Installer ==="
echo "Voice dictation for Linux"
echo

# Check for required tools
echo "Checking dependencies..."

MISSING=()

command -v curl >/dev/null 2>&1 || MISSING+=("curl")
command -v jq >/dev/null 2>&1 || MISSING+=("jq")
command -v xdotool >/dev/null 2>&1 || MISSING+=("xdotool")
command -v ffmpeg >/dev/null 2>&1 || MISSING+=("ffmpeg")
command -v pw-record >/dev/null 2>&1 || MISSING+=("pipewire")
command -v notify-send >/dev/null 2>&1 || MISSING+=("libnotify")
command -v yad >/dev/null 2>&1 || MISSING+=("yad")

if [[ ${#MISSING[@]} -gt 0 ]]; then
    echo "Missing packages: ${MISSING[*]}"
    echo
    echo "Install them with:"
    echo "  Arch/Manjaro: sudo pacman -S ${MISSING[*]}"
    echo "  Ubuntu/Debian: sudo apt install ${MISSING[*]}"
    echo "  Fedora: sudo dnf install ${MISSING[*]}"
    echo
    read -p "Try to install now? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        if command -v pacman >/dev/null 2>&1; then
            sudo pacman -S --noconfirm "${MISSING[@]}"
        elif command -v apt >/dev/null 2>&1; then
            sudo apt install -y "${MISSING[@]}"
        elif command -v dnf >/dev/null 2>&1; then
            sudo dnf install -y "${MISSING[@]}"
        else
            echo "Unknown package manager. Please install manually."
            exit 1
        fi
    else
        exit 1
    fi
fi

echo "All dependencies installed!"
echo

# Create .env from example if it doesn't exist
if [[ ! -f "$SCRIPT_DIR/.env" ]]; then
    if [[ -f "$SCRIPT_DIR/.env.example" ]]; then
        cp "$SCRIPT_DIR/.env.example" "$SCRIPT_DIR/.env"
        echo "Created .env from .env.example"
    fi
fi

# Check .env configuration
source "$SCRIPT_DIR/.env" 2>/dev/null || true
if [[ -z "$GROQ_API_KEY" ]]; then
    echo ""
    echo "WARNING: You need to configure your Groq API key!"
    echo ""
    echo "  1. Get a free API key from: https://console.groq.com/keys"
    echo "  2. Edit: $SCRIPT_DIR/.env"
    echo "  3. Set GROQ_API_KEY=\"your-key-here\""
    echo ""
    read -p "Open .env in editor now? [Y/n] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        ${EDITOR:-nano} "$SCRIPT_DIR/.env"
    fi
fi

# Create systemd user service
echo "Creating systemd user service..."
mkdir -p ~/.config/systemd/user

cat > ~/.config/systemd/user/fluistern.service << EOF
[Unit]
Description=Flüstern Voice Dictation Daemon
After=graphical-session.target

[Service]
Type=simple
ExecStart=$SCRIPT_DIR/voice-input-daemon.sh
Restart=on-failure
RestartSec=3
Environment=DISPLAY=:0

[Install]
WantedBy=default.target
EOF

systemctl --user daemon-reload
echo "Service created: fluistern.service"
echo

# Enable and start service
read -p "Enable and start the daemon now? [Y/n] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Nn]$ ]]; then
    systemctl --user enable fluistern.service
    systemctl --user start fluistern.service
    echo "Daemon started! Tray icon should appear."
fi

echo
echo "=== Installation Complete ==="
echo
echo "Next steps:"
echo
echo "  1. Right-click the tray icon to configure mic/language"
echo
echo "  2. Add a keybinding to your WM:"
echo
echo "     dwm config.h:"
echo "       { MODKEY, XK_r, spawn, SHCMD(\"$SCRIPT_DIR/voice-input.sh\") },"
echo
echo "     i3 config:"
echo "       bindsym \$mod+r exec $SCRIPT_DIR/voice-input.sh"
echo
echo "     sxhkd:"
echo "       super + r"
echo "           $SCRIPT_DIR/voice-input.sh"
echo
echo "Commands:"
echo "  Start daemon:  systemctl --user start fluistern"
echo "  Stop daemon:   systemctl --user stop fluistern"
echo "  View logs:     journalctl --user -u fluistern -f"
echo
