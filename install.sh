#!/bin/bash
# Flüstern Installation Script
# Voice dictation for Linux using Groq Whisper API

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$HOME/.local/share/fluistern"
BIN_DIR="$HOME/.local/bin"

echo "=== Flüstern Installer ==="
echo "Voice dictation for Linux"
echo

# Check for required tools
echo "Checking dependencies..."

MISSING=()

command -v curl >/dev/null 2>&1 || MISSING+=("curl")
command -v jq >/dev/null 2>&1 || MISSING+=("jq")
command -v xdotool >/dev/null 2>&1 || MISSING+=("xdotool")
command -v xclip >/dev/null 2>&1 || MISSING+=("xclip")
command -v ffmpeg >/dev/null 2>&1 || MISSING+=("ffmpeg")
command -v pw-record >/dev/null 2>&1 || MISSING+=("pipewire")
command -v notify-send >/dev/null 2>&1 || MISSING+=("libnotify")
command -v yad >/dev/null 2>&1 || MISSING+=("yad")
command -v python3 >/dev/null 2>&1 || MISSING+=("python3")
command -v sqlite3 >/dev/null 2>&1 || MISSING+=("sqlite3")

# Check Python GTK dependencies
if command -v python3 >/dev/null 2>&1; then
    python3 -c "import gi; gi.require_version('Gtk', '4.0'); gi.require_version('Adw', '1')" 2>/dev/null || MISSING+=("python-gobject" "libadwaita")
fi

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

# Install files to ~/.local/share/fluistern
echo "Installing to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
mkdir -p "$BIN_DIR"

# Copy all files
cp -r "$SCRIPT_DIR/icons" "$INSTALL_DIR/"
cp "$SCRIPT_DIR/voice-input.sh" "$INSTALL_DIR/"
cp "$SCRIPT_DIR/voice-input-daemon.sh" "$INSTALL_DIR/"
cp "$SCRIPT_DIR/select-mic.sh" "$INSTALL_DIR/"
cp "$SCRIPT_DIR/select-language.sh" "$INSTALL_DIR/"
cp "$SCRIPT_DIR/gui.py" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR"/*.sh
chmod +x "$INSTALL_DIR/gui.py"

# Create .env from example if it doesn't exist
if [[ ! -f "$INSTALL_DIR/.env" ]]; then
    if [[ -f "$SCRIPT_DIR/.env.example" ]]; then
        cp "$SCRIPT_DIR/.env.example" "$INSTALL_DIR/.env"
    elif [[ -f "$SCRIPT_DIR/.env" ]]; then
        cp "$SCRIPT_DIR/.env" "$INSTALL_DIR/.env"
    fi
fi

# Create symlink in ~/.local/bin
ln -sf "$INSTALL_DIR/voice-input.sh" "$BIN_DIR/fluistern"
echo "Created symlink: $BIN_DIR/fluistern"

# Check .env configuration
source "$INSTALL_DIR/.env" 2>/dev/null || true
if [[ -z "$GROQ_API_KEY" ]]; then
    echo ""
    echo "Configure your Groq API key:"
    echo ""
    echo "  1. Get a free key from: https://console.groq.com/keys"
    echo "  2. Edit: $INSTALL_DIR/.env"
    echo ""
    read -p "Open .env in nano now? [Y/n] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        if command -v nano >/dev/null 2>&1; then
            nano "$INSTALL_DIR/.env"
        else
            ${EDITOR:-vi} "$INSTALL_DIR/.env"
        fi
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
ExecStart=$INSTALL_DIR/voice-input-daemon.sh
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
echo "Installed to: $INSTALL_DIR"
echo "Command: fluistern (or $BIN_DIR/fluistern)"
echo
echo "You can now delete the git clone folder if you want."
echo
echo "Next steps:"
echo
echo "  1. Right-click the tray icon to configure mic/language"
echo
echo "  2. Add a keybinding in your WM/DE config to run: fluistern"
echo
echo "     Examples:"
echo "       sxhkd:     super + r -> fluistern"
echo "       Hyprland:  bind = SUPER, R, exec, fluistern"
echo "       i3/sway:   bindsym \$mod+r exec fluistern"
echo "       dwm:       { MODKEY, XK_r, spawn, SHCMD(\"fluistern\") }"
echo
echo "Commands:"
echo "  Start daemon:  systemctl --user start fluistern"
echo "  Stop daemon:   systemctl --user stop fluistern"
echo "  Uninstall:     rm -rf $INSTALL_DIR $BIN_DIR/fluistern"
echo
