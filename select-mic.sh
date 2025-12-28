#!/bin/bash
# Microphone selection helper

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="$SCRIPT_DIR/.env"
ICON_DIR="$SCRIPT_DIR/icons"

source "$ENV_FILE"

# Get available microphones (non-monitor sources)
get_mics() {
    pactl list sources short | grep -v monitor | awk '{print $2}'
}

# Build menu items
mics=$(get_mics)
current_mic="${MIC_SOURCE:-}"

menu_items=""
# Add default option first
if [[ -z "$current_mic" ]]; then
    menu_items="TRUE\n(default)\n"
else
    menu_items="FALSE\n(default)\n"
fi

for mic in $mics; do
    if [[ "$mic" == "$current_mic" ]]; then
        menu_items="${menu_items}TRUE\n${mic}\n"
    else
        menu_items="${menu_items}FALSE\n${mic}\n"
    fi
done

# Show selection dialog
selected=$(echo -e "$menu_items" | yad --list \
    --title="Select Microphone" \
    --column="":RD \
    --column="Microphone" \
    --radiolist \
    --width=500 \
    --height=300 \
    --print-column=2 \
    --separator="" \
    --center \
    2>/dev/null)

if [[ -n "$selected" ]]; then
    # Update .env file
    if [[ "$selected" == "(default)" ]]; then
        sed -i 's/^MIC_SOURCE=.*/MIC_SOURCE=""/' "$ENV_FILE"
        notify-send "Voice Input" "Using default microphone" -i "$ICON_DIR/idle.svg"
    else
        sed -i "s/^MIC_SOURCE=.*/MIC_SOURCE=\"$selected\"/" "$ENV_FILE"
        notify-send "Voice Input" "Microphone: $selected" -i "$ICON_DIR/idle.svg"
    fi
fi
