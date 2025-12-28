# Flüstern

> WhisperFlow-style voice dictation for Linux (dwm/X11)

A lightweight voice-to-text tool that uses Groq's Whisper API for transcription and LLM for formatting. Works with any X11 window manager.

## Features

- **Fast transcription** using Groq's Whisper Large V3 Turbo
- **Smart formatting** with automatic punctuation and capitalization
- **System tray icon** with status indicators
- **Language selection** for 13+ languages
- **Microphone selection** via tray menu
- **Keyboard shortcut** toggle (start/stop recording)
- **Types directly** into any focused text field

## Demo

```
[Press Win+R] → Recording...
[Speak] "hello world this is a test"
[Press Win+R] → Processing...
[Output] "Hello world, this is a test."
```

## Requirements

- Linux with X11 (tested on dwm, i3, etc.)
- PipeWire or PulseAudio
- Groq API key (free tier available)

## Installation

```bash
# Clone the repo
git clone https://github.com/YOUR_USERNAME/fluistern.git
cd fluistern

# Copy and configure .env
cp .env.example .env
nano .env  # Add your GROQ_API_KEY

# Run installer
./install.sh
```

## Dependencies

The installer will check for these and offer to install them:

- `yad` - tray icon
- `xdotool` - typing into windows
- `ffmpeg` - audio compression
- `jq` - JSON parsing
- `curl` - API calls
- `pw-record` (PipeWire) or `parecord` (PulseAudio)

## Configuration

### Get Groq API Key

1. Go to [console.groq.com/keys](https://console.groq.com/keys)
2. Create a free account
3. Generate an API key
4. Add it to `.env`

### dwm Keybinding

Add to your `config.h`:

```c
{ MODKEY, XK_r, spawn, SHCMD("/path/to/fluistern/voice-input.sh") },
```

### i3 Keybinding

Add to `~/.config/i3/config`:

```
bindsym $mod+r exec /path/to/fluistern/voice-input.sh
```

### sxhkd (bspwm, etc.)

Add to `~/.config/sxhkd/sxhkdrc`:

```
super + r
    /path/to/fluistern/voice-input.sh
```

## Usage

1. Start the daemon: `systemctl --user start voice-input`
2. Press your keybind to start recording
3. Speak
4. Press keybind again to stop and transcribe
5. Text appears in your focused window

### Tray Menu (right-click)

- **Toggle Recording** - Start/stop recording
- **Select Microphone** - Choose input device
- **Select Language** - Set transcription language
- **Quit** - Stop the daemon

## How It Works

```
┌─────────────────────────────────────────────────┐
│  Keybind (Win+R)                                │
│       ↓                                         │
│  Start pw-record (16kHz mono WAV)               │
│       ↓                                         │
│  Keybind again → Stop recording                 │
│       ↓                                         │
│  Compress to Opus (48kbps, ~30x smaller)        │
│       ↓                                         │
│  Upload to Groq Whisper API                     │
│       ↓                                         │
│  Format with Groq LLM (punctuation, caps)       │
│       ↓                                         │
│  xdotool types into focused window              │
└─────────────────────────────────────────────────┘
```

## Files

```
fluistern/
├── voice-input.sh          # Main toggle script
├── voice-input-daemon.sh   # Tray daemon
├── select-mic.sh           # Microphone selector
├── select-language.sh      # Language selector
├── install.sh              # Installer
├── .env.example            # Config template
└── icons/
    ├── idle.svg            # Grey mic
    ├── recording.svg       # Red mic
    └── processing.svg      # Yellow mic
```

## Troubleshooting

### No tray icon
- Install `yad`: `sudo pacman -S yad`
- Make sure you have a system tray (trayer, stalonetray, etc.)

### Transcription errors
- Set specific language in tray menu (auto-detect can miss)
- Speak clearly, reduce background noise
- Check microphone selection

### Slow processing
- Mostly upload time to Groq API
- Keep recordings under 30 seconds for <5s processing

## License

MIT

## Credits

- [Groq](https://groq.com) - Fast Whisper API
- [OpenAI Whisper](https://github.com/openai/whisper) - Speech recognition model
- Inspired by [WhisperFlow](https://github.com/...) for macOS
