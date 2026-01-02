# Flüstern

> WhisperFlow-style voice dictation for Linux

A lightweight voice-to-text tool that uses Groq's Whisper API for transcription and LLM for formatting. Works with any X11/Wayland window manager.

## Features

- **Fast transcription** using Groq's Whisper Large V3 Turbo
- **Smart formatting** with automatic punctuation and capitalization
- **Rust-powered GUI** - CPU-optimized settings & history interface
- **System tray icon** with status indicators
- **50+ languages** including Hindi, Arabic, Chinese, and more
- **Microphone selection** via tray menu
- **Keyboard shortcut** toggle (start/stop recording)
- **Pastes directly** into any focused text field
- **Recording history** with corrections and timing data

## Demo

```
[Press shortcut] → Recording...
[Speak] "hello world this is a test"
[Press shortcut] → Processing...
[Output] "Hello world, this is a test."
```

## Requirements

- Linux with X11 or Wayland
- PipeWire or PulseAudio
- Rust/Cargo (for building GUI)
- GTK4 and libadwaita (for GUI)
- Groq API key (free tier available)

## Installation

```bash
git clone https://github.com/chukfinley/fluistern.git
cd fluistern
./install.sh
```

The installer will:
1. Check and install dependencies (if needed)
2. Build the Rust GUI (Release mode, optimized)
3. Copy files to `~/.local/share/fluistern/`
4. Create `fluistern` command in `~/.local/bin/`
5. Set up systemd service
6. Prompt for Groq API key

After install, you can delete the cloned folder.

## Dependencies

The installer will check for these:

**Required:**
- `cargo` / `rust` - Rust toolchain for building GUI
- `libgtk-4-dev` - GTK4 development libraries
- `libadwaita-1-dev` - libadwaita development libraries
- `yad` - tray icon
- `xdotool` - simulating paste
- `xclip` - clipboard access
- `ffmpeg` - audio compression
- `jq` - JSON parsing
- `curl` - API calls
- `pw-record` (PipeWire)
- `sqlite3` - database for recording history

**On Ubuntu/Debian:**
```bash
sudo apt install cargo libgtk-4-dev libadwaita-1-dev yad xdotool xclip ffmpeg jq curl pipewire sqlite3
```

**On Arch/Manjaro:**
```bash
sudo pacman -S rust gtk4 libadwaita yad xdotool xclip ffmpeg jq curl pipewire sqlite3
```

## Configuration

### Get Groq API Key

1. Go to [console.groq.com/keys](https://console.groq.com/keys)
2. Create a free account
3. Generate an API key
4. The installer will prompt you, or edit `~/.local/share/fluistern/.env`

### Config Options

Edit `~/.local/share/fluistern/.env`:

```bash
GROQ_API_KEY="your-key"      # Required
LANGUAGE="de"                 # Optional: en, de, es, fr, hi, etc.
MIC_SOURCE=""                 # Optional: specific mic (use tray menu to select)
NOTIFICATIONS="true"          # Show notifications (true/false)
TRAY_ICON="true"              # Show tray icon (true/false)
```

### Add Keybinding

Add a keybinding in your WM config to run `fluistern`:

| WM/DE | Config | Example |
|-------|--------|---------|
| sxhkd | `~/.config/sxhkd/sxhkdrc` | `super + r` <br> `    fluistern` |
| Hyprland | `~/.config/hypr/hyprland.conf` | `bind = SUPER, R, exec, fluistern` |
| i3/sway | `~/.config/i3/config` | `bindsym $mod+r exec fluistern` |
| dwm | `config.h` | `{ MODKEY, XK_r, spawn, SHCMD("fluistern") }` |

## Usage

1. Start the daemon: `systemctl --user start fluistern`
2. Press your keybind to start recording
3. Speak
4. Press keybind again to stop and transcribe
5. Text appears in your focused window

### Tray Menu (right-click)

- **Toggle Recording** - Start/stop recording
- **Einstellungen & Historie** - Open GUI for settings, logs, and recording history
- **Select Microphone** - Choose input device
- **Select Language** - Set transcription language (50+ languages)
- **Quit** - Stop the daemon

### GUI Features

Open the GUI from the tray menu to access:
- **Recording History** - View all recordings with transcriptions and timings
- **Corrections** - Teach the system by correcting misheard words
- **Debug Logs** - Real-time logs for troubleshooting
- **Settings** - Configure API key, language, microphone, system prompt

## Supported Languages

Auto-detect, English, German, Spanish, French, Hindi, Italian, Portuguese, Dutch, Polish, Russian, Chinese, Japanese, Korean, Arabic, Turkish, Vietnamese, Thai, Indonesian, Ukrainian, Czech, Greek, Hebrew, Hungarian, Swedish, Danish, Finnish, Norwegian, Romanian, Bengali, Tamil, Telugu, Urdu, Persian, and many more.

## How It Works

```
Keybind → Start recording (16kHz mono WAV)
Keybind → Stop → Compress to Opus (~30x smaller)
       → Upload to Groq Whisper API
       → Format with Groq LLM (punctuation, caps)
       → Pastes into focused window (Ctrl+V)
```

## Files

After installation:
```
~/.local/share/fluistern/
├── fluistern-gui           # Rust GUI binary (settings, history, logs)
├── voice-input.sh          # Main toggle script
├── voice-input-daemon.sh   # Tray daemon
├── select-mic.sh           # Microphone selector
├── select-language.sh      # Language selector
├── .env                    # Your config (API key, language, mic)
├── history.db              # SQLite database for recordings
└── icons/                  # Tray icons

~/.local/bin/fluistern      # Symlink to run the tool
```

## Commands

```bash
fluistern                           # Toggle recording
systemctl --user start fluistern    # Start daemon
systemctl --user stop fluistern     # Stop daemon
systemctl --user status fluistern   # Check status
journalctl --user -u fluistern -f   # View logs
```

## Uninstall

```bash
systemctl --user stop fluistern
systemctl --user disable fluistern
rm -rf ~/.local/share/fluistern ~/.local/bin/fluistern
rm ~/.config/systemd/user/fluistern.service
```

## Troubleshooting

### No tray icon
- Install `yad`: `sudo pacman -S yad`
- Make sure you have a system tray

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
