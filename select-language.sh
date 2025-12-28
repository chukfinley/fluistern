#!/bin/bash
# Language selection helper

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="$SCRIPT_DIR/.env"
ICON_DIR="$SCRIPT_DIR/icons"

source "$ENV_FILE" 2>/dev/null

# Common languages for Whisper
LANGUAGES=(
    "auto|Auto-detect"
    "en|English"
    "de|German (Deutsch)"
    "es|Spanish (Español)"
    "fr|French (Français)"
    "it|Italian (Italiano)"
    "pt|Portuguese (Português)"
    "nl|Dutch (Nederlands)"
    "pl|Polish (Polski)"
    "ru|Russian (Русский)"
    "zh|Chinese (中文)"
    "ja|Japanese (日本語)"
    "ko|Korean (한국어)"
)

# Build menu items
menu_items=""
current_lang="${LANGUAGE:-auto}"

for lang_entry in "${LANGUAGES[@]}"; do
    code="${lang_entry%%|*}"
    name="${lang_entry#*|}"
    
    if [[ "$code" == "$current_lang" ]] || [[ "$code" == "auto" && -z "$LANGUAGE" ]]; then
        menu_items="${menu_items}TRUE\n${name}\n${code}\n"
    else
        menu_items="${menu_items}FALSE\n${name}\n${code}\n"
    fi
done

# Show selection dialog
selected=$(echo -e "$menu_items" | yad --list \
    --title="Select Language" \
    --column="":RD \
    --column="Language" \
    --column="Code":HD \
    --radiolist \
    --width=350 \
    --height=400 \
    --print-column=3 \
    --separator="" \
    --center \
    2>/dev/null)

if [[ -n "$selected" ]]; then
    # Update .env file
    if [[ "$selected" == "auto" ]]; then
        if grep -q "^LANGUAGE=" "$ENV_FILE" 2>/dev/null; then
            sed -i 's/^LANGUAGE=.*/LANGUAGE=""/' "$ENV_FILE"
        fi
        notify-send "Voice Input" "Language: Auto-detect" -i "$ICON_DIR/idle.svg"
    else
        if grep -q "^LANGUAGE=" "$ENV_FILE" 2>/dev/null; then
            sed -i "s/^LANGUAGE=.*/LANGUAGE=\"$selected\"/" "$ENV_FILE"
        else
            echo "LANGUAGE=\"$selected\"" >> "$ENV_FILE"
        fi
        # Find the display name
        for lang_entry in "${LANGUAGES[@]}"; do
            code="${lang_entry%%|*}"
            name="${lang_entry#*|}"
            if [[ "$code" == "$selected" ]]; then
                notify-send "Voice Input" "Language: $name" -i "$ICON_DIR/idle.svg"
                break
            fi
        done
    fi
fi
