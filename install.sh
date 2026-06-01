#!/bin/bash
# Plauder Installation Script
# Voice dictation for Linux using Groq Whisper API

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$HOME/.local/share/plauder"
BIN_DIR="$HOME/.local/bin"

echo "=== Plauder Installer ==="
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
command -v sqlite3 >/dev/null 2>&1 || MISSING+=("sqlite3")
command -v cargo >/dev/null 2>&1 || MISSING+=("cargo" "rust")
command -v node >/dev/null 2>&1 || MISSING+=("nodejs")
command -v pnpm >/dev/null 2>&1 || MISSING+=("pnpm")

# Tauri needs the WebKitGTK 4.1 runtime/dev libs to build the GUI.
if ! pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
    echo "Note: WebKitGTK 4.1 dev libs are required to build the Tauri GUI."
    echo "  Arch:          sudo pacman -S webkit2gtk-4.1 libappindicator-gtk3 librsvg"
    echo "  Ubuntu/Debian: sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev \\"
    echo "                   libayatana-appindicator3-dev librsvg2-dev"
    echo "  Fedora:        sudo dnf install webkit2gtk4.1-devel openssl-devel \\"
    echo "                   libappindicator-gtk3-devel librsvg2-devel"
    echo
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

# Build Tauri GUI (React frontend + Rust backend)
echo "Building GUI (Tauri + React)..."
cd "$SCRIPT_DIR"
pnpm install
if ! pnpm tauri build --no-bundle; then
    echo "Failed to build Tauri GUI"
    exit 1
fi
GUI_BIN="$SCRIPT_DIR/src-tauri/target/release/plauder-gui"
if [[ ! -f "$GUI_BIN" ]]; then
    echo "GUI binary not found at $GUI_BIN"
    exit 1
fi
echo "GUI built successfully!"
echo

# Install files to ~/.local/share/plauder
echo "Installing to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
mkdir -p "$BIN_DIR"

# Copy all files
cp -r "$SCRIPT_DIR/icons" "$INSTALL_DIR/"
cp "$SCRIPT_DIR/voice-input.sh" "$INSTALL_DIR/"
cp "$SCRIPT_DIR/voice-input-daemon.sh" "$INSTALL_DIR/"
cp "$SCRIPT_DIR/select-mic.sh" "$INSTALL_DIR/"
cp "$SCRIPT_DIR/select-language.sh" "$INSTALL_DIR/"
cp "$GUI_BIN" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR"/*.sh
chmod +x "$INSTALL_DIR/plauder-gui"

# Create .env from example if it doesn't exist
if [[ ! -f "$INSTALL_DIR/.env" ]]; then
    if [[ -f "$SCRIPT_DIR/.env.example" ]]; then
        cp "$SCRIPT_DIR/.env.example" "$INSTALL_DIR/.env"
    elif [[ -f "$SCRIPT_DIR/.env" ]]; then
        cp "$SCRIPT_DIR/.env" "$INSTALL_DIR/.env"
    fi
fi

# Create symlink in ~/.local/bin
ln -sf "$INSTALL_DIR/voice-input.sh" "$BIN_DIR/plauder"
echo "Created symlink: $BIN_DIR/plauder"

# Install desktop entry + icon so the GUI shows up in app launchers
echo "Installing launcher entry..."
APP_DIR="$HOME/.local/share/applications"
HICOLOR="$HOME/.local/share/icons/hicolor"
mkdir -p "$APP_DIR" "$HICOLOR/scalable/apps"

# Install the SVG as the scalable launcher icon so high-DPI displays render
# crisply. PNG sizes go alongside as theme fallbacks.
cp "$SCRIPT_DIR/icons/plauder.svg" "$HICOLOR/scalable/apps/plauder.svg"
for size in 16 32 48 64 128 256; do
    src="$SCRIPT_DIR/src-tauri/icons/${size}x${size}.png"
    if [[ ! -f "$src" ]]; then
        case "$size" in
            16|48|64|256) src="" ;;
            32)  src="$SCRIPT_DIR/src-tauri/icons/32x32.png" ;;
            128) src="$SCRIPT_DIR/src-tauri/icons/128x128.png" ;;
        esac
    fi
    [[ -n "$src" && -f "$src" ]] || continue
    mkdir -p "$HICOLOR/${size}x${size}/apps"
    cp "$src" "$HICOLOR/${size}x${size}/apps/plauder.png"
done
cp "$SCRIPT_DIR/src-tauri/icons/icon.png" "$INSTALL_DIR/icon.png"
cp "$SCRIPT_DIR/icons/plauder.svg" "$INSTALL_DIR/icon.svg"

# Generate the .desktop with absolute paths (Exec= does NOT expand $HOME).
# Icon= references the hicolor theme so the launcher picks the best size.
cat > "$APP_DIR/plauder.desktop" <<EOF
[Desktop Entry]
Name=Plauder
GenericName=Voice Dictation
Comment=Recording history, logs & settings for voice input
Exec=$INSTALL_DIR/plauder-gui
Icon=plauder
Terminal=false
Type=Application
Categories=AudioVideo;Audio;Utility;
Keywords=voice;input;dictation;whisper;transcription;plauder;
StartupWMClass=Plauder
EOF
chmod 644 "$APP_DIR/plauder.desktop"
# Remove old/broken entries if present
rm -f "$APP_DIR/plauder-gui.desktop" "$APP_DIR/fluistern-gui.desktop" "$APP_DIR/fluistern.desktop"
update-desktop-database "$APP_DIR" 2>/dev/null || true
gtk-update-icon-cache -f -t "$HICOLOR" 2>/dev/null || true
echo "Launcher entry installed: Plauder"

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

cat > ~/.config/systemd/user/plauder.service << EOF
[Unit]
Description=Plauder Voice Dictation Daemon
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
echo "Service created: plauder.service"
echo

# Enable and start service
read -p "Enable and start the daemon now? [Y/n] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Nn]$ ]]; then
    systemctl --user enable plauder.service
    systemctl --user start plauder.service
    echo "Daemon started! Tray icon should appear."
fi

echo
echo "=== Installation Complete ==="
echo
echo "Installed to: $INSTALL_DIR"
echo "Command: plauder (or $BIN_DIR/plauder)"
echo
echo "You can now delete the git clone folder if you want."
echo
echo "Next steps:"
echo
echo "  1. Right-click the tray icon to configure mic/language"
echo
echo "  2. Add a keybinding in your WM/DE config to run: plauder"
echo
echo "     Examples:"
echo "       sxhkd:     super + r -> plauder"
echo "       Hyprland:  bind = SUPER, R, exec, plauder"
echo "       i3/sway:   bindsym \$mod+r exec plauder"
echo "       dwm:       { MODKEY, XK_r, spawn, SHCMD(\"plauder\") }"
echo
echo "Commands:"
echo "  Start daemon:  systemctl --user start plauder"
echo "  Stop daemon:   systemctl --user stop plauder"
echo "  Uninstall:     rm -rf $INSTALL_DIR $BIN_DIR/plauder"
echo
