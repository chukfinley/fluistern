<p align="center">
  <img src="icons/plauder.svg" alt="Plauder" width="160" height="160"/>
</p>

<h1 align="center">Plauder</h1>

<p align="center">
  <em>WhisperFlow-style voice dictation for Linux — Groq Whisper + LLM formatting, pastes into any focused window.</em>
</p>

<p align="center">
  <a href="https://github.com/chukfinley/plauder/actions/workflows/build.yml"><img src="https://github.com/chukfinley/plauder/actions/workflows/build.yml/badge.svg" alt="Build"/></a>
  <a href="https://github.com/chukfinley/plauder/releases/latest"><img src="https://img.shields.io/github/v/release/chukfinley/plauder" alt="Release"/></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"/></a>
</p>

Press a keybind, speak, press again — the text appears in whatever window has focus. Transcription via Groq's Whisper Large V3 Turbo, optional clean-up via a Groq LLM, history kept locally in SQLite.

```
[Press shortcut] → Recording…
[Speak]          "hello world this is a test"
[Press shortcut] → Processing…
[Output]         "Hello world, this is a test."
```

## Features

- **Fast transcription** — Whisper Large V3 Turbo on Groq, sub-second for short clips
- **Clean output** — Groq LLM adds punctuation/casing without rewriting your words
- **Never swallows text** — truncated or refusal-style LLM output is detected and replaced with the raw transcript
- **Tiny upload** — opusenc 16 kbps VOIP, ~4 KB per second of audio (~10× smaller than the previous ffmpeg/48k pipeline)
- **Modern GUI** — Tauri 2 + React 19 + Tailwind v4, dark UI, system tray, lives in `~/.local/share/plauder`
- **Verlauf tab** — virtualized history list with git-style word diff between Whisper raw and LLM formatted output, character-level change percentage, anomaly badges (refusal / truncated / inflated)
- **Editable corrections** — teach Plauder by adding "Whisper hears X → I meant Y" pairs; applied to every future LLM call
- **Live Logs tab** — last 100 lines by default, scroll up to auto-load older
- **API-key reveal** — Eye/EyeOff toggle in Settings instead of a permanent password field
- **50+ languages** — auto-detect or pin via the Settings/tray dropdown
- **Keybind-driven** — works with i3/sway/Hyprland/sxhkd/dwm

## Installation

### Pre-built `.deb` / `.AppImage`

Every tagged release ships a Debian package and an AppImage on the [Releases page](https://github.com/chukfinley/plauder/releases/latest).

```bash
# .deb
sudo apt install ./plauder_*_amd64.deb

# AppImage
chmod +x plauder_*.AppImage
./plauder_*.AppImage
```

### From source

```bash
git clone https://github.com/chukfinley/plauder.git
cd plauder
./install.sh
```

The installer builds the Tauri + React GUI in release mode, copies everything to `~/.local/share/plauder/`, drops a `plauder` symlink in `~/.local/bin/`, sets up the systemd user service, and prompts for your Groq API key.

## Requirements

- Linux with X11 or Wayland
- PipeWire or PulseAudio (`pw-record`)
- Rust/Cargo + Node.js + pnpm (build from source only)
- WebKitGTK 4.1 dev libs (Tauri runtime)
- Groq API key — [console.groq.com/keys](https://console.groq.com/keys)

### System dependencies

**Required at runtime:**
- `yad` — tray icon
- `xdotool` — paste into focused window
- `xclip` — clipboard
- `opusenc` (recommended) or `ffmpeg` — audio compression
- `jq` `curl` `sqlite3`
- `pw-record` (PipeWire)

**Ubuntu/Debian:**
```bash
sudo apt install yad xdotool xclip opus-tools ffmpeg jq curl pipewire sqlite3 \
  libwebkit2gtk-4.1-dev build-essential libssl-dev libayatana-appindicator3-dev librsvg2-dev
# build-from-source extras:
sudo apt install cargo nodejs && sudo npm install -g pnpm
```

**Arch/Manjaro:**
```bash
sudo pacman -S yad xdotool xclip opus-tools ffmpeg jq curl pipewire sqlite3 \
  webkit2gtk-4.1 libappindicator-gtk3 librsvg
# build-from-source extras:
sudo pacman -S rust nodejs pnpm
```

## Configuration

Edit `~/.local/share/plauder/.env` or use the **Settings** tab in the GUI:

```bash
GROQ_API_KEY="gsk_…"     # Required
LANGUAGE="de"            # Optional: auto-detect if empty
MIC_SOURCE=""            # Optional: device id from `pactl list sources short`
NOTIFICATIONS="true"
TRAY_ICON="true"
SYSTEM_PROMPT="…"        # Optional: override the LLM clean-up prompt
```

### Keybindings

Bind your WM/DE to run `plauder`:

| WM/DE     | Config                          | Example                                                    |
|-----------|----------------------------------|------------------------------------------------------------|
| i3 / sway | `~/.config/i3/config`            | `bindsym $mod+r exec plauder`                              |
| Regolith  | `~/.config/regolith3/i3/config.d/91_custom` | `bindsym $mod+r exec plauder`                   |
| Hyprland  | `~/.config/hypr/hyprland.conf`   | `bind = SUPER, R, exec, plauder`                           |
| sxhkd     | `~/.config/sxhkd/sxhkdrc`        | `super + r` <br> `    plauder`                             |
| dwm       | `config.h`                       | `{ MODKEY, XK_r, spawn, SHCMD("plauder") }`                |

## Usage

1. `systemctl --user start plauder` (tray daemon)
2. Press your keybind → recording
3. Speak
4. Press keybind again → transcribe + paste

### GUI tabs

- **Verlauf** — every recording, expandable card with Whisper raw / LLM formatted / your correction, git-style word diff, anomaly badges, % changed, copy/delete
- **Korrekturen** — add and edit "Whisper hears X → I meant Y" rules used by every future LLM call
- **Logs** — live debug log, last 100 lines, scroll up for older
- **Einstellungen** — API key (with show/hide), mic, language, notifications, tray, system prompt

### Tray menu

- Toggle Recording
- Einstellungen & Historie (opens GUI)
- Select Microphone
- Select Language
- Quit

## How it works

```
Keybind → pw-record 16 kHz mono WAV → /tmp
Keybind → opusenc 16 kbps VOIP (~16 ms encode, ~4 KB/s upload)
        → POST /audio/transcriptions  (Whisper Large V3 Turbo)
        → POST /chat/completions      (gpt-oss-20b, low reasoning effort)
        → xdotool pastes into the focused window
        → SQLite history.db gets the row + timings
```

**Anti-swallow** safeguard: if the LLM finishes with `finish_reason=length`, returns less than 60 % of the input, or matches one of the known refusal patterns ("kann ich nicht", "I cannot", "as an AI", …), Plauder pastes the raw Whisper transcript instead.

## Files after install

```
~/.local/share/plauder/
├── plauder-gui              # Tauri release binary
├── voice-input.sh           # toggle script (recording + transcribe + paste)
├── voice-input-daemon.sh    # tray daemon
├── select-mic.sh / select-language.sh
├── .env                     # config (API key, language, mic)
├── history.db               # SQLite history + corrections
└── icons/                   # tray + window icons

~/.local/bin/plauder         # symlink → voice-input.sh
~/.config/systemd/user/plauder.service
```

## Development

Tauri 2 app: React/Vite frontend in `src/`, Rust backend in `src-tauri/`.

```bash
pnpm install
pnpm tauri dev                            # hot-reload
pnpm tauri build --no-bundle              # binary → src-tauri/target/release/plauder-gui
pnpm tauri build                          # full .deb + .rpm + AppImage bundle
```

Data lives in the same `history.db` and `.env` the shell scripts use, so the GUI and CLI stay in sync.

## Releases

Push a `vX.Y.Z` tag and the GitHub Action builds `.deb`, `.rpm`, and `.AppImage` and attaches them to a new release automatically.

```bash
git tag v0.2.0
git push origin v0.2.0
```

## Uninstall

```bash
systemctl --user stop plauder && systemctl --user disable plauder
rm -rf ~/.local/share/plauder ~/.local/bin/plauder
rm ~/.config/systemd/user/plauder.service
```

## Troubleshooting

- **No tray icon** — install `yad`, make sure your DE has a system tray
- **Empty / wrong transcript** — pin the language in Settings (auto-detect can miss for short clips)
- **Slow** — mostly Groq upload + LLM round-trip; with opusenc 16 kbps a 10 s clip uploads in < 200 ms on WiFi
- **Tray panic on launch** — make sure `~/.local/share/plauder/icons/32x32.png` exists; the binary loads it via `include_bytes!` at build time

## License

MIT — see [LICENSE](LICENSE).

## Credits

- [Groq](https://groq.com) — fast Whisper + LLM inference
- [OpenAI Whisper](https://github.com/openai/whisper) — transcription model
- [Tauri](https://tauri.app) — Rust + WebView shell
