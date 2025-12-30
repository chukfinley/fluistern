#!/usr/bin/env python3
"""
Flüstern GUI - Recording History, Debug Logs & Settings
"""

import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Adw, GLib, Gio, Pango
import sqlite3
import os
import json
from datetime import datetime
from pathlib import Path

# Find the script directory (handles symlinks)
SCRIPT_DIR = Path(__file__).resolve().parent
DB_FILE = SCRIPT_DIR / "history.db"
ENV_FILE = SCRIPT_DIR / ".env"
LOG_FILE = Path("/tmp/voice-input-debug.log")

# Default system prompt
DEFAULT_SYSTEM_PROMPT = """You are a dictation formatter. Add proper punctuation (periods, commas, question marks) and fix capitalization (sentence starts, proper nouns). Do NOT add any markdown, asterisks, bold, or formatting. Output the plain corrected text only, nothing else."""


class Database:
    """SQLite database for recording history"""

    def __init__(self):
        self.conn = sqlite3.connect(str(DB_FILE))
        self.conn.row_factory = sqlite3.Row
        self.create_tables()

    def create_tables(self):
        cursor = self.conn.cursor()
        cursor.execute('''
            CREATE TABLE IF NOT EXISTS recordings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                whisper_output TEXT,
                llm_output TEXT,
                user_correction TEXT,
                audio_duration_ms INTEGER,
                whisper_duration_ms INTEGER,
                llm_duration_ms INTEGER,
                total_duration_ms INTEGER,
                success INTEGER DEFAULT 1,
                error_message TEXT
            )
        ''')
        cursor.execute('''
            CREATE TABLE IF NOT EXISTS corrections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                whisper_pattern TEXT NOT NULL,
                intended_text TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
        ''')
        self.conn.commit()

    def add_recording(self, whisper_output, llm_output, audio_duration_ms=0,
                      whisper_duration_ms=0, llm_duration_ms=0, total_duration_ms=0,
                      success=True, error_message=None):
        cursor = self.conn.cursor()
        cursor.execute('''
            INSERT INTO recordings
            (timestamp, whisper_output, llm_output, audio_duration_ms,
             whisper_duration_ms, llm_duration_ms, total_duration_ms, success, error_message)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ''', (
            datetime.now().isoformat(),
            whisper_output,
            llm_output,
            audio_duration_ms,
            whisper_duration_ms,
            llm_duration_ms,
            total_duration_ms,
            1 if success else 0,
            error_message
        ))
        self.conn.commit()
        return cursor.lastrowid

    def update_correction(self, recording_id, user_correction):
        cursor = self.conn.cursor()
        cursor.execute('''
            UPDATE recordings SET user_correction = ? WHERE id = ?
        ''', (user_correction, recording_id))
        self.conn.commit()

        # Also save as a correction pattern
        recording = self.get_recording(recording_id)
        if recording and recording['whisper_output']:
            cursor.execute('''
                INSERT INTO corrections (whisper_pattern, intended_text, created_at)
                VALUES (?, ?, ?)
            ''', (recording['whisper_output'], user_correction, datetime.now().isoformat()))
            self.conn.commit()

    def get_recording(self, recording_id):
        cursor = self.conn.cursor()
        cursor.execute('SELECT * FROM recordings WHERE id = ?', (recording_id,))
        return cursor.fetchone()

    def get_all_recordings(self, limit=100):
        cursor = self.conn.cursor()
        cursor.execute('''
            SELECT * FROM recordings ORDER BY timestamp DESC LIMIT ?
        ''', (limit,))
        return cursor.fetchall()

    def get_corrections(self):
        cursor = self.conn.cursor()
        cursor.execute('SELECT * FROM corrections ORDER BY created_at DESC')
        return cursor.fetchall()

    def export_corrections_for_prompt(self):
        """Export corrections as context for the LLM"""
        corrections = self.get_corrections()
        if not corrections:
            return ""

        lines = ["\n\nUser correction patterns (use these to better understand what the user means):"]
        for c in corrections[:20]:  # Limit to 20 most recent
            lines.append(f'- When transcribed as "{c["whisper_pattern"]}", the user meant: "{c["intended_text"]}"')
        return "\n".join(lines)

    def close(self):
        self.conn.close()


class EnvConfig:
    """Handle .env configuration file"""

    def __init__(self):
        self.config = {}
        self.load()

    def load(self):
        self.config = {
            'GROQ_API_KEY': '',
            'MIC_SOURCE': '',
            'LANGUAGE': '',
            'NOTIFICATIONS': 'true',
            'TRAY_ICON': 'true',
            'SYSTEM_PROMPT': DEFAULT_SYSTEM_PROMPT,
        }

        if ENV_FILE.exists():
            with open(ENV_FILE, 'r') as f:
                for line in f:
                    line = line.strip()
                    if line and not line.startswith('#') and '=' in line:
                        key, value = line.split('=', 1)
                        # Remove quotes
                        value = value.strip().strip('"').strip("'")
                        self.config[key] = value

    def save(self):
        lines = [
            "# Voice Input Configuration",
            "# Get your Groq API key from: https://console.groq.com/keys",
            f'GROQ_API_KEY="{self.config.get("GROQ_API_KEY", "")}"',
            "",
            "# Selected microphone source (leave empty for default, or set via tray menu)",
            "# Run 'pactl list sources short' to see available sources",
            f'MIC_SOURCE="{self.config.get("MIC_SOURCE", "")}"',
            "",
            "# Language for transcription (e.g., \"de\" for German, \"en\" for English)",
            "# Leave empty for auto-detect",
            f'LANGUAGE="{self.config.get("LANGUAGE", "")}"',
            "",
            "# Show notifications (true/false, default: true)",
            f'NOTIFICATIONS="{self.config.get("NOTIFICATIONS", "true")}"',
            "",
            "# Show tray icon (true/false, default: true)",
            f'TRAY_ICON="{self.config.get("TRAY_ICON", "true")}"',
            "",
            "# System prompt for LLM formatting (customize to improve output)",
            f'SYSTEM_PROMPT="{self.config.get("SYSTEM_PROMPT", DEFAULT_SYSTEM_PROMPT)}"',
            "",
        ]

        with open(ENV_FILE, 'w') as f:
            f.write('\n'.join(lines))

    def get(self, key, default=''):
        return self.config.get(key, default)

    def set(self, key, value):
        self.config[key] = value


class RecordingRow(Gtk.Box):
    """A single recording entry with inline editing"""

    def __init__(self, recording, db, on_save_callback):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.recording = dict(recording)
        self.db = db
        self.on_save_callback = on_save_callback
        self.expanded = False

        self.add_css_class("card")
        self.set_margin_top(6)
        self.set_margin_bottom(6)
        self.set_margin_start(12)
        self.set_margin_end(12)

        # Header row (always visible) - clickable to expand
        self.header_btn = Gtk.Button()
        self.header_btn.add_css_class("flat")
        self.header_btn.connect("clicked", self._toggle_expand)

        header_content = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        header_content.set_margin_top(12)
        header_content.set_margin_bottom(12)
        header_content.set_margin_start(12)
        header_content.set_margin_end(12)

        # Expand arrow
        self.expand_icon = Gtk.Image.new_from_icon_name("pan-end-symbolic")
        header_content.append(self.expand_icon)

        # Timestamp
        timestamp = self.recording.get('timestamp', '')
        try:
            dt = datetime.fromisoformat(timestamp)
            time_str = dt.strftime("%d.%m. %H:%M")
        except:
            time_str = "?"

        time_label = Gtk.Label(label=time_str)
        time_label.add_css_class("dim-label")
        time_label.set_width_chars(12)
        header_content.append(time_label)

        # Preview of LLM output (or whisper if no LLM)
        preview_text = self.recording.get('llm_output') or self.recording.get('whisper_output') or ""
        preview = preview_text[:60] + ("..." if len(preview_text) > 60 else "")
        preview_label = Gtk.Label(label=preview)
        preview_label.set_xalign(0)
        preview_label.set_hexpand(True)
        preview_label.set_ellipsize(Pango.EllipsizeMode.END)
        header_content.append(preview_label)

        # Status / correction indicator
        if self.recording.get('user_correction'):
            status_label = Gtk.Label(label="korrigiert")
            status_label.add_css_class("success")
        elif self.recording.get('success'):
            status_label = Gtk.Label(label="")
        else:
            status_label = Gtk.Label(label="Fehler")
            status_label.add_css_class("error")
        status_label.add_css_class("caption")
        header_content.append(status_label)

        self.header_btn.set_child(header_content)
        self.append(self.header_btn)

        # Expandable content area
        self.detail_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        self.detail_box.set_margin_start(16)
        self.detail_box.set_margin_end(16)
        self.detail_box.set_margin_bottom(16)
        self.detail_box.set_visible(False)

        # Duration info
        total_ms = self.recording.get('total_duration_ms') or 0
        whisper_ms = self.recording.get('whisper_duration_ms') or 0
        llm_ms = self.recording.get('llm_duration_ms') or 0
        if total_ms > 0:
            timing_label = Gtk.Label(label=f"Dauer: {total_ms}ms (Whisper: {whisper_ms}ms, LLM: {llm_ms}ms)")
            timing_label.add_css_class("caption")
            timing_label.add_css_class("dim-label")
            timing_label.set_xalign(0)
            self.detail_box.append(timing_label)

        # Whisper output section
        whisper_out = self.recording.get('whisper_output') or ""
        whisper_group = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        whisper_header = Gtk.Label(label="Whisper (Rohtranskription)")
        whisper_header.add_css_class("heading")
        whisper_header.set_xalign(0)
        whisper_group.append(whisper_header)

        whisper_frame = Gtk.Frame()
        whisper_scroll = Gtk.ScrolledWindow()
        whisper_scroll.set_min_content_height(60)
        whisper_scroll.set_max_content_height(120)
        self.whisper_view = Gtk.TextView()
        self.whisper_view.set_editable(False)
        self.whisper_view.set_wrap_mode(Gtk.WrapMode.WORD)
        self.whisper_view.set_cursor_visible(False)
        self.whisper_view.get_buffer().set_text(whisper_out)
        self.whisper_view.set_margin_top(8)
        self.whisper_view.set_margin_bottom(8)
        self.whisper_view.set_margin_start(8)
        self.whisper_view.set_margin_end(8)
        whisper_scroll.set_child(self.whisper_view)
        whisper_frame.set_child(whisper_scroll)
        whisper_group.append(whisper_frame)
        self.detail_box.append(whisper_group)

        # LLM output section
        llm_out = self.recording.get('llm_output') or ""
        llm_group = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        llm_header = Gtk.Label(label="LLM (Formatiert)")
        llm_header.add_css_class("heading")
        llm_header.set_xalign(0)
        llm_group.append(llm_header)

        llm_frame = Gtk.Frame()
        llm_scroll = Gtk.ScrolledWindow()
        llm_scroll.set_min_content_height(60)
        llm_scroll.set_max_content_height(120)
        self.llm_view = Gtk.TextView()
        self.llm_view.set_editable(False)
        self.llm_view.set_wrap_mode(Gtk.WrapMode.WORD)
        self.llm_view.set_cursor_visible(False)
        self.llm_view.get_buffer().set_text(llm_out)
        self.llm_view.set_margin_top(8)
        self.llm_view.set_margin_bottom(8)
        self.llm_view.set_margin_start(8)
        self.llm_view.set_margin_end(8)
        llm_scroll.set_child(self.llm_view)
        llm_frame.set_child(llm_scroll)
        llm_group.append(llm_frame)
        self.detail_box.append(llm_group)

        # Correction section (editable!)
        corr_group = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        corr_header = Gtk.Label(label="Deine Korrektur (was du eigentlich meintest)")
        corr_header.add_css_class("heading")
        corr_header.add_css_class("accent")
        corr_header.set_xalign(0)
        corr_group.append(corr_header)

        corr_frame = Gtk.Frame()
        corr_frame.add_css_class("accent-border")
        corr_scroll = Gtk.ScrolledWindow()
        corr_scroll.set_min_content_height(80)
        corr_scroll.set_max_content_height(150)
        self.corr_view = Gtk.TextView()
        self.corr_view.set_editable(True)
        self.corr_view.set_wrap_mode(Gtk.WrapMode.WORD)
        user_corr = self.recording.get('user_correction') or ""
        # Pre-fill with LLM output if no correction yet
        self.corr_view.get_buffer().set_text(user_corr if user_corr else llm_out)
        self.corr_view.set_margin_top(8)
        self.corr_view.set_margin_bottom(8)
        self.corr_view.set_margin_start(8)
        self.corr_view.set_margin_end(8)
        corr_scroll.set_child(self.corr_view)
        corr_frame.set_child(corr_scroll)
        corr_group.append(corr_frame)

        # Save button
        save_btn = Gtk.Button(label="Korrektur speichern")
        save_btn.add_css_class("suggested-action")
        save_btn.connect("clicked", self._on_save)
        corr_group.append(save_btn)

        self.detail_box.append(corr_group)

        # Error message if any
        err_msg = self.recording.get('error_message') or ""
        if err_msg:
            err_label = Gtk.Label(label=f"Fehler: {err_msg}")
            err_label.add_css_class("error")
            err_label.set_xalign(0)
            err_label.set_wrap(True)
            self.detail_box.append(err_label)

        self.append(self.detail_box)

    def _toggle_expand(self, button):
        self.expanded = not self.expanded
        self.detail_box.set_visible(self.expanded)
        if self.expanded:
            self.expand_icon.set_from_icon_name("pan-down-symbolic")
        else:
            self.expand_icon.set_from_icon_name("pan-end-symbolic")

    def _on_save(self, button):
        buffer = self.corr_view.get_buffer()
        start, end = buffer.get_bounds()
        correction = buffer.get_text(start, end, False).strip()

        if correction:
            recording_id = self.recording.get('id')
            self.db.update_correction(recording_id, correction)
            self.on_save_callback()


class FluesternGUI(Adw.Application):
    """Main application"""

    def __init__(self):
        super().__init__(application_id="de.fluistern.gui")
        self.db = Database()
        self.config = EnvConfig()
        self.connect("activate", self.on_activate)

    def on_activate(self, app):
        # Create main window
        self.win = Adw.ApplicationWindow(application=app)
        self.win.set_title("Flüstern")
        self.win.set_default_size(900, 700)

        # Main layout
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)

        # Header bar
        header = Adw.HeaderBar()

        # Refresh button
        refresh_btn = Gtk.Button(icon_name="view-refresh-symbolic")
        refresh_btn.set_tooltip_text("Aktualisieren")
        refresh_btn.connect("clicked", self.on_refresh)
        header.pack_start(refresh_btn)

        main_box.append(header)

        # Tab view
        self.stack = Adw.ViewStack()

        # History page
        history_page = self.create_history_page()
        self.stack.add_titled(history_page, "history", "Historie")

        # Logs page
        logs_page = self.create_logs_page()
        self.stack.add_titled(logs_page, "logs", "Debug Logs")

        # Settings page
        settings_page = self.create_settings_page()
        self.stack.add_titled(settings_page, "settings", "Einstellungen")

        # Corrections page
        corrections_page = self.create_corrections_page()
        self.stack.add_titled(corrections_page, "corrections", "Korrekturen")

        # View switcher
        switcher = Adw.ViewSwitcher()
        switcher.set_stack(self.stack)
        switcher.set_policy(Adw.ViewSwitcherPolicy.WIDE)
        header.set_title_widget(switcher)

        main_box.append(self.stack)

        self.win.set_content(main_box)

        # Load CSS
        self.load_css()

        # Start log watcher
        self.start_log_watcher()

        self.win.present()

    def load_css(self):
        css = b"""
        .success { color: #26a269; }
        .error { color: #e01b24; }
        .accent { color: #3584e4; font-weight: bold; }
        .mono { font-family: monospace; font-size: 11px; }
        .log-entry { padding: 4px 8px; }
        .card {
            background: alpha(@card_bg_color, 0.8);
            border-radius: 12px;
            border: 1px solid alpha(@borders, 0.5);
        }
        .heading {
            font-weight: bold;
            font-size: 0.9em;
        }
        .accent-border {
            border: 2px solid #3584e4;
            border-radius: 8px;
        }
        textview {
            background: transparent;
        }
        textview text {
            background: transparent;
        }
        """
        provider = Gtk.CssProvider()
        provider.load_from_data(css)
        Gtk.StyleContext.add_provider_for_display(
            self.win.get_display(),
            provider,
            Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )

    def create_history_page(self):
        """Create the recording history page"""
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)

        self.history_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        scrolled.set_child(self.history_box)

        self.refresh_history()

        return scrolled

    def refresh_history(self):
        # Clear existing items
        while child := self.history_box.get_first_child():
            self.history_box.remove(child)

        # Add recordings
        recordings = self.db.get_all_recordings()

        if not recordings:
            empty_label = Gtk.Label(label="Keine Aufnahmen vorhanden.\nStarte eine Aufnahme mit dem Voice Input Toggle.")
            empty_label.set_margin_top(50)
            empty_label.add_css_class("dim-label")
            self.history_box.append(empty_label)
        else:
            for rec in recordings:
                row = RecordingRow(rec, self.db, self._on_correction_saved)
                self.history_box.append(row)

    def _on_correction_saved(self):
        """Called when a correction is saved - refresh the corrections tab"""
        self.refresh_corrections()

    def create_logs_page(self):
        """Create the debug logs page"""
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        box.set_margin_top(12)
        box.set_margin_bottom(12)
        box.set_margin_start(12)
        box.set_margin_end(12)

        # Info label
        info = Gtk.Label(label="Debug-Logs vom Voice Input Script")
        info.add_css_class("dim-label")
        info.set_xalign(0)
        box.append(info)

        # Clear button
        clear_btn = Gtk.Button(label="Logs löschen")
        clear_btn.connect("clicked", self.on_clear_logs)
        box.append(clear_btn)

        # Log view
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)

        self.log_view = Gtk.TextView()
        self.log_view.set_editable(False)
        self.log_view.set_monospace(True)
        self.log_view.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        self.log_buffer = self.log_view.get_buffer()

        scrolled.set_child(self.log_view)
        box.append(scrolled)

        self.refresh_logs()

        return box

    def refresh_logs(self):
        if LOG_FILE.exists():
            with open(LOG_FILE, 'r') as f:
                content = f.read()
            self.log_buffer.set_text(content)
        else:
            self.log_buffer.set_text("Keine Logs vorhanden.\n\nLogs werden beim nächsten Voice Input erstellt.")

    def on_clear_logs(self, button):
        if LOG_FILE.exists():
            LOG_FILE.unlink()
        self.log_buffer.set_text("")

    def start_log_watcher(self):
        """Watch log file for changes"""
        def check_logs():
            self.refresh_logs()
            return True
        GLib.timeout_add_seconds(2, check_logs)

    def create_settings_page(self):
        """Create the settings page"""
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=16)
        box.set_margin_top(20)
        box.set_margin_bottom(20)
        box.set_margin_start(20)
        box.set_margin_end(20)

        # API Key
        api_group = Adw.PreferencesGroup(title="API Konfiguration")

        self.api_entry = Adw.EntryRow(title="Groq API Key")
        self.api_entry.set_text(self.config.get('GROQ_API_KEY', ''))
        api_group.add(self.api_entry)
        box.append(api_group)

        # Recording settings
        rec_group = Adw.PreferencesGroup(title="Aufnahme")

        self.mic_entry = Adw.EntryRow(title="Mikrofon Source (leer = Standard)")
        self.mic_entry.set_text(self.config.get('MIC_SOURCE', ''))
        rec_group.add(self.mic_entry)

        self.lang_entry = Adw.EntryRow(title="Sprache (z.B. 'de', 'en', leer = auto)")
        self.lang_entry.set_text(self.config.get('LANGUAGE', ''))
        rec_group.add(self.lang_entry)

        box.append(rec_group)

        # UI settings
        ui_group = Adw.PreferencesGroup(title="Oberfläche")

        self.notif_switch = Adw.SwitchRow(title="Benachrichtigungen")
        self.notif_switch.set_active(self.config.get('NOTIFICATIONS', 'true').lower() == 'true')
        ui_group.add(self.notif_switch)

        self.tray_switch = Adw.SwitchRow(title="Tray Icon")
        self.tray_switch.set_active(self.config.get('TRAY_ICON', 'true').lower() == 'true')
        ui_group.add(self.tray_switch)

        box.append(ui_group)

        # System prompt
        prompt_group = Adw.PreferencesGroup(title="System Prompt")
        prompt_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)

        prompt_info = Gtk.Label(label="Der System Prompt wird dem LLM gegeben, um die Formatierung zu steuern:")
        prompt_info.add_css_class("dim-label")
        prompt_info.set_xalign(0)
        prompt_info.set_wrap(True)
        prompt_box.append(prompt_info)

        prompt_scroll = Gtk.ScrolledWindow()
        prompt_scroll.set_min_content_height(150)

        self.prompt_view = Gtk.TextView()
        self.prompt_view.set_wrap_mode(Gtk.WrapMode.WORD)
        prompt_buffer = self.prompt_view.get_buffer()
        prompt_buffer.set_text(self.config.get('SYSTEM_PROMPT', DEFAULT_SYSTEM_PROMPT))
        prompt_scroll.set_child(self.prompt_view)
        prompt_box.append(prompt_scroll)

        reset_prompt_btn = Gtk.Button(label="Standard wiederherstellen")
        reset_prompt_btn.connect("clicked", self.on_reset_prompt)
        prompt_box.append(reset_prompt_btn)

        prompt_group.add(prompt_box)
        box.append(prompt_group)

        # Save button
        save_btn = Gtk.Button(label="Einstellungen speichern")
        save_btn.add_css_class("suggested-action")
        save_btn.connect("clicked", self.on_save_settings)
        box.append(save_btn)

        scrolled.set_child(box)
        return scrolled

    def on_reset_prompt(self, button):
        buffer = self.prompt_view.get_buffer()
        buffer.set_text(DEFAULT_SYSTEM_PROMPT)

    def on_save_settings(self, button):
        self.config.set('GROQ_API_KEY', self.api_entry.get_text())
        self.config.set('MIC_SOURCE', self.mic_entry.get_text())
        self.config.set('LANGUAGE', self.lang_entry.get_text())
        self.config.set('NOTIFICATIONS', 'true' if self.notif_switch.get_active() else 'false')
        self.config.set('TRAY_ICON', 'true' if self.tray_switch.get_active() else 'false')

        prompt_buffer = self.prompt_view.get_buffer()
        start, end = prompt_buffer.get_bounds()
        self.config.set('SYSTEM_PROMPT', prompt_buffer.get_text(start, end, False))

        self.config.save()

        # Show confirmation
        toast = Adw.Toast(title="Einstellungen gespeichert!")
        toast.set_timeout(2)
        # Note: Need toast overlay for this to work properly
        print("Settings saved!")

    def create_corrections_page(self):
        """Create the corrections/training page"""
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_margin_top(12)
        box.set_margin_bottom(12)
        box.set_margin_start(12)
        box.set_margin_end(12)

        # Info
        info = Gtk.Label()
        info.set_markup("<b>Gespeicherte Korrekturen</b>\n\nDiese Korrekturen werden dem LLM als Kontext gegeben,\num deine Aussprache besser zu verstehen.")
        info.set_xalign(0)
        info.set_wrap(True)
        box.append(info)

        # Export button
        export_btn = Gtk.Button(label="Korrekturen als Prompt-Kontext exportieren")
        export_btn.connect("clicked", self.on_export_corrections)
        box.append(export_btn)

        # List
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)

        self.corrections_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        scrolled.set_child(self.corrections_box)
        box.append(scrolled)

        self.refresh_corrections()

        return box

    def refresh_corrections(self):
        # Clear
        while child := self.corrections_box.get_first_child():
            self.corrections_box.remove(child)

        corrections = self.db.get_corrections()

        if not corrections:
            empty = Gtk.Label(label="Noch keine Korrekturen.\n\nKlicke bei einer Aufnahme auf 'Korrigieren' um zu trainieren.")
            empty.add_css_class("dim-label")
            empty.set_margin_top(30)
            self.corrections_box.append(empty)
        else:
            for c in corrections:
                c_dict = dict(c)  # Convert sqlite3.Row to dict
                row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
                row.set_margin_start(8)
                row.set_margin_end(8)
                row.set_margin_top(4)
                row.set_margin_bottom(4)

                pattern = Gtk.Label(label=f'"{c_dict.get("whisper_pattern", "")}"')
                pattern.add_css_class("dim-label")
                pattern.set_xalign(0)
                row.append(pattern)

                arrow = Gtk.Label(label="->")
                row.append(arrow)

                intended = Gtk.Label(label=f'"{c_dict.get("intended_text", "")}"')
                intended.add_css_class("accent")
                intended.set_xalign(0)
                row.append(intended)

                self.corrections_box.append(row)

    def on_export_corrections(self, button):
        context = self.db.export_corrections_for_prompt()
        if context:
            # Copy to clipboard
            clipboard = self.win.get_clipboard()
            clipboard.set(context)
            print("Corrections exported to clipboard!")
            print(context)
        else:
            print("No corrections to export")

    def on_refresh(self, button):
        self.refresh_history()
        self.refresh_logs()
        self.refresh_corrections()


def main():
    app = FluesternGUI()
    app.run(None)


if __name__ == "__main__":
    main()
