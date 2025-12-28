#!/bin/bash
# Voice Input Daemon - Manages tray icon
# Run this at startup (e.g., in .xinitrc or as systemd user service)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ICON_DIR="$SCRIPT_DIR/icons"
PIPE_FILE="/tmp/voice-input-pipe"
STATE_FILE="/tmp/voice-input-state"
ICON_STATE_FILE="/tmp/voice-input-icon-state"

# Cleanup on exit
cleanup() {
    rm -f "$PIPE_FILE" "$ICON_STATE_FILE"
    exit 0
}
trap cleanup EXIT INT TERM

# Create named pipe for communication
rm -f "$PIPE_FILE"
mkfifo "$PIPE_FILE"

# Initialize icon state
echo "idle" > "$ICON_STATE_FILE"

echo "Voice Input daemon started"
echo "Tray icon running - right-click for menu, left-click to toggle recording"

# Run yad with menu - this blocks until quit
yad --notification \
    --image="$ICON_DIR/idle.svg" \
    --text="Voice Input - Ready" \
    --menu="Toggle Recording!$SCRIPT_DIR/voice-input.sh|\
Select Microphone!$SCRIPT_DIR/select-mic.sh|\
Select Language!$SCRIPT_DIR/select-language.sh|\
Quit!quit" \
    --command="$SCRIPT_DIR/voice-input.sh" &

YAD_PID=$!

# Background process to update icon based on state file
while kill -0 $YAD_PID 2>/dev/null; do
    sleep 0.5

    # Check if state file changed
    if [[ -f "$STATE_FILE" ]]; then
        # Recording in progress
        if [[ "$(cat "$ICON_STATE_FILE" 2>/dev/null)" != "recording" ]]; then
            echo "recording" > "$ICON_STATE_FILE"
            notify-send "Voice Input" "Recording..." -i "$ICON_DIR/recording.svg" -t 1500
        fi
    else
        if [[ "$(cat "$ICON_STATE_FILE" 2>/dev/null)" != "idle" ]]; then
            echo "idle" > "$ICON_STATE_FILE"
        fi
    fi
done

cleanup
