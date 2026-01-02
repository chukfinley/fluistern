#!/bin/bash
# Voice Input Daemon - Manages tray icon (optional)
# Run this at startup (e.g., in .xinitrc or as systemd user service)

# Resolve symlinks to find real script directory
SCRIPT_PATH="${BASH_SOURCE[0]}"
while [[ -L "$SCRIPT_PATH" ]]; do
    SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
    SCRIPT_PATH="$(readlink "$SCRIPT_PATH")"
    [[ "$SCRIPT_PATH" != /* ]] && SCRIPT_PATH="$SCRIPT_DIR/$SCRIPT_PATH"
done
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"

# Load config
source "$SCRIPT_DIR/.env" 2>/dev/null

# Defaults
TRAY_ICON="${TRAY_ICON:-true}"
NOTIFICATIONS="${NOTIFICATIONS:-true}"

ICON_DIR="$SCRIPT_DIR/icons"
STATE_FILE="/tmp/voice-input-state"
ICON_STATE_FILE="/tmp/voice-input-icon-state"

# Cleanup on exit
cleanup() {
    rm -f "$ICON_STATE_FILE"
    exit 0
}
trap cleanup EXIT INT TERM

echo "Flüstern daemon started"

# If tray icon disabled, just keep daemon alive
if [[ "$TRAY_ICON" != "true" ]]; then
    echo "Tray icon disabled, running in background only"
    while true; do
        sleep 3600
    done
fi

# Tray icon enabled - run yad
echo "Tray icon running - right-click for menu"

# Initialize icon state
echo "idle" > "$ICON_STATE_FILE"

# Run yad with menu
yad --notification \
    --image="$ICON_DIR/idle.svg" \
    --text="Flüstern - Ready" \
    --menu="Toggle Recording!$SCRIPT_DIR/voice-input.sh|\
Einstellungen & Historie!$SCRIPT_DIR/fluistern-gui|\
Select Microphone!$SCRIPT_DIR/select-mic.sh|\
Select Language!$SCRIPT_DIR/select-language.sh|\
Quit!quit" \
    --command="$SCRIPT_DIR/voice-input.sh" &

YAD_PID=$!

# Background process to update icon based on state file
while kill -0 $YAD_PID 2>/dev/null; do
    sleep 0.5

    if [[ -f "$STATE_FILE" ]]; then
        if [[ "$(cat "$ICON_STATE_FILE" 2>/dev/null)" != "recording" ]]; then
            echo "recording" > "$ICON_STATE_FILE"
            # Notification is sent by voice-input.sh, not here (avoid duplicates)
        fi
    else
        if [[ "$(cat "$ICON_STATE_FILE" 2>/dev/null)" != "idle" ]]; then
            echo "idle" > "$ICON_STATE_FILE"
        fi
    fi
done

cleanup
