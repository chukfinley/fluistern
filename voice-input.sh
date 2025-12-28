#!/bin/bash
# Voice Input - Toggle script
# Call once to start recording, call again to stop and transcribe

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="$SCRIPT_DIR/.env"
STATE_FILE="/tmp/voice-input-state"
AUDIO_FILE="/tmp/voice-input-recording.wav"
AUDIO_COMPRESSED="/tmp/voice-input-recording.ogg"
PIPE_FILE="/tmp/voice-input-pipe"

# Load config
source "$ENV_FILE"

# Function to update tray state
update_tray() {
    if [[ -p "$PIPE_FILE" ]]; then
        echo "state:$1" > "$PIPE_FILE" 2>/dev/null &
    fi
}

# Function to show notification
notify() {
    notify-send "Voice Input" "$1" -i "$SCRIPT_DIR/icons/$2.svg" -t 2000
}

# Function to compress audio to opus/ogg (small but good quality)
compress_audio() {
    ffmpeg -y -i "$AUDIO_FILE" -ar 16000 -ac 1 -c:a libopus -b:a 48k "$AUDIO_COMPRESSED" 2>/dev/null
}

# Function to transcribe audio using Groq
transcribe() {
    local response
    local lang_param=""

    # Add language parameter if set
    if [[ -n "$LANGUAGE" ]]; then
        lang_param="-F language=$LANGUAGE"
    fi

    response=$(curl -s -X POST "https://api.groq.com/openai/v1/audio/transcriptions" \
        -H "Authorization: Bearer $GROQ_API_KEY" \
        -F "file=@$AUDIO_COMPRESSED" \
        -F "model=whisper-large-v3-turbo" \
        -F "response_format=text" \
        $lang_param)

    echo "$response"
}

# Function to format text using Groq (openai/gpt-oss-20b)
format_text() {
    local text="$1"
    local response

    # Use jq to properly escape the text for JSON
    local json_payload
    json_payload=$(jq -n \
        --arg text "$text" \
        '{
            "model": "openai/gpt-oss-20b",
            "messages": [
                {
                    "role": "system",
                    "content": "You are a dictation formatter. Add proper punctuation (periods, commas, question marks) and fix capitalization (sentence starts, proper nouns). Do NOT add any markdown, asterisks, bold, or formatting. Output the plain corrected text only, nothing else."
                },
                {
                    "role": "user",
                    "content": $text
                }
            ],
            "temperature": 0.1
        }')

    response=$(curl -s -X POST "https://api.groq.com/openai/v1/chat/completions" \
        -H "Authorization: Bearer $GROQ_API_KEY" \
        -H "Content-Type: application/json" \
        -d "$json_payload")

    # Extract the content from the response
    echo "$response" | jq -r '.choices[0].message.content // empty'
}

# Function to type text into focused window
type_text() {
    local text="$1"
    # Small delay to ensure focus returns to original window
    sleep 0.1
    # Use xdotool to type the text
    xdotool type --clearmodifiers --delay 5 -- "$text"
}

# Main toggle logic
if [[ -f "$STATE_FILE" ]]; then
    # Currently recording - stop and process
    PID=$(cat "$STATE_FILE")
    
    # Stop recording
    kill "$PID" 2>/dev/null
    wait "$PID" 2>/dev/null
    rm -f "$STATE_FILE"
    
    update_tray "processing"
    notify "Processing..." "processing"
    
    # Check if audio file exists and has content
    if [[ ! -f "$AUDIO_FILE" ]] || [[ ! -s "$AUDIO_FILE" ]]; then
        notify "No audio recorded" "idle"
        update_tray "idle"
        exit 1
    fi

    # Compress to mp3 for fast upload
    compress_audio

    # Transcribe
    transcript=$(transcribe)
    
    if [[ -z "$transcript" ]]; then
        notify "Transcription failed" "idle"
        update_tray "idle"
        rm -f "$AUDIO_FILE" "$AUDIO_COMPRESSED"
        exit 1
    fi
    
    # Format text
    formatted=$(format_text "$transcript")
    
    if [[ -z "$formatted" ]]; then
        # If formatting fails, use raw transcript
        formatted="$transcript"
    fi
    
    # Type the result
    type_text "$formatted"
    
    # Cleanup
    rm -f "$AUDIO_FILE" "$AUDIO_COMPRESSED"
    
    update_tray "idle"
    notify "Done!" "idle"
else
    # Start recording
    update_tray "recording"
    notify "Recording..." "recording"
    
    # Determine mic source
    if [[ -n "$MIC_SOURCE" ]]; then
        SOURCE_ARG="--target=$MIC_SOURCE"
    else
        SOURCE_ARG=""
    fi
    
    # Remove old audio file
    rm -f "$AUDIO_FILE"
    
    # Start recording in background (16kHz mono for smaller files)
    pw-record --rate 16000 --channels 1 $SOURCE_ARG "$AUDIO_FILE" &
    RECORD_PID=$!
    
    # Save PID to state file
    echo "$RECORD_PID" > "$STATE_FILE"
fi
