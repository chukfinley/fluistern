mod config;
mod database;

use chrono::DateTime;
use config::EnvConfig;
use database::{Database, Recording};
use glib::clone;
use gtk4::prelude::*;
use gtk4::{glib, Application, ApplicationWindow};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::SystemTime;

const APP_ID: &str = "de.fluistern.gui";

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(build_ui);

    app.run()
}

fn get_script_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn get_db_file() -> PathBuf {
    let mut path = get_script_dir();
    path.push("history.db");
    path
}

fn get_env_file() -> PathBuf {
    let mut path = get_script_dir();
    path.push(".env");
    path
}

fn get_log_file() -> PathBuf {
    PathBuf::from("/tmp/voice-input-debug.log")
}

fn is_tiling_wm() -> bool {
    // Check XDG_CURRENT_DESKTOP
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        let desktop_lower = desktop.to_lowercase();
        if desktop_lower.is_empty() {
            return true;
        }

        let tiling_wms = [
            "dwm", "i3", "bspwm", "sway", "hyprland", "awesome", "xmonad", "qtile",
            "herbstluftwm", "river", "leftwm",
        ];

        if tiling_wms.iter().any(|wm| desktop_lower.contains(wm)) {
            return true;
        }
    }

    // Try wmctrl
    if let Ok(output) = std::process::Command::new("wmctrl")
        .arg("-m")
        .output()
    {
        let wm_name = String::from_utf8_lossy(&output.stdout).to_lowercase();
        let tiling_wms = [
            "dwm", "i3", "bspwm", "sway", "hyprland", "awesome", "xmonad", "qtile",
            "herbstluftwm", "river", "leftwm",
        ];
        if tiling_wms.iter().any(|wm| wm_name.contains(wm)) {
            return true;
        }
    }

    false
}

struct AppState {
    db: Arc<Database>,
    config: Rc<RefCell<EnvConfig>>,
    history_box: gtk4::Box,
    corrections_box: gtk4::Box,
    log_view: gtk4::TextView,
    log_watcher_id: Rc<RefCell<Option<glib::SourceId>>>,
    last_log_mtime: Rc<RefCell<SystemTime>>,
}

fn build_ui(app: &Application) {
    adw::init().expect("Failed to initialize libadwaita");

    let db = Arc::new(Database::new(get_db_file()).expect("Failed to open database"));
    let config = Rc::new(RefCell::new(EnvConfig::new(get_env_file())));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Flüstern")
        .default_width(900)
        .default_height(700)
        .build();

    let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // Header bar
    let header = adw::HeaderBar::new();
    if is_tiling_wm() {
        header.set_show_end_title_buttons(false);
        header.set_show_start_title_buttons(false);
    }

    // Refresh button
    let refresh_btn = gtk4::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh")
        .build();
    header.pack_start(&refresh_btn);

    main_box.append(&header);

    // Tab view
    let stack = adw::ViewStack::new();

    // Create pages
    let history_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let history_scroll = gtk4::ScrolledWindow::builder()
        .vexpand(true)
        .child(&history_box)
        .build();
    stack.add_titled(&history_scroll, Some("history"), "History");

    let (logs_page, log_view) = create_logs_page();
    stack.add_titled(&logs_page, Some("logs"), "Debug Logs");

    let (settings_page, settings_widgets) = create_settings_page(config.clone());
    stack.add_titled(&settings_page, Some("settings"), "Settings");

    let corrections_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    let corrections_page = create_corrections_page(corrections_box.clone());
    stack.add_titled(&corrections_page, Some("corrections"), "Corrections");

    // View switcher
    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();
    header.set_title_widget(Some(&switcher));

    main_box.append(&stack);
    window.set_child(Some(&main_box));

    // Load CSS
    load_css(&window);

    let state = Rc::new(AppState {
        db: db.clone(),
        config: config.clone(),
        history_box: history_box.clone(),
        corrections_box: corrections_box.clone(),
        log_view: log_view.clone(),
        log_watcher_id: Rc::new(RefCell::new(None)),
        last_log_mtime: Rc::new(RefCell::new(SystemTime::UNIX_EPOCH)),
    });

    // Refresh history
    refresh_history(&state);
    refresh_corrections(&state);

    // Start log watcher
    start_log_watcher(state.clone());

    // Refresh button handler
    refresh_btn.connect_clicked(clone!(@strong state => move |_| {
        refresh_history(&state);
        refresh_logs(&state);
        refresh_corrections(&state);
    }));

    // Connect settings save button
    settings_widgets
        .save_btn
        .connect_clicked(clone!(@strong config, @strong settings_widgets => move |_| {
            let mut cfg = config.borrow_mut();
            cfg.set("GROQ_API_KEY".to_string(), settings_widgets.api_entry.text().to_string());
            cfg.set("MIC_SOURCE".to_string(), settings_widgets.mic_entry.text().to_string());
            cfg.set("LANGUAGE".to_string(), settings_widgets.lang_entry.text().to_string());
            cfg.set("NOTIFICATIONS".to_string(), if settings_widgets.notif_switch.is_active() { "true" } else { "false" }.to_string());
            cfg.set("TRAY_ICON".to_string(), if settings_widgets.tray_switch.is_active() { "true" } else { "false" }.to_string());

            let buffer = settings_widgets.prompt_view.buffer();
            let prompt = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
            cfg.set("SYSTEM_PROMPT".to_string(), prompt.to_string());

            if let Err(e) = cfg.save() {
                eprintln!("Failed to save settings: {}", e);
            } else {
                println!("Settings saved!");
            }
        }));

    settings_widgets
        .reset_prompt_btn
        .connect_clicked(clone!(@strong settings_widgets => move |_| {
            settings_widgets.prompt_view.buffer().set_text(EnvConfig::get_default_system_prompt());
        }));

    settings_widgets
        .clear_logs_btn
        .connect_clicked(clone!(@strong state => move |_| {
            let log_file = get_log_file();
            if log_file.exists() {
                let _ = std::fs::remove_file(log_file);
            }
            state.log_view.buffer().set_text("");
        }));

    // Cleanup on window close
    window.connect_close_request(clone!(@strong state => move |_| {
        if let Some(id) = state.log_watcher_id.borrow_mut().take() {
            id.remove();
        }
        glib::Propagation::Proceed
    }));

    window.present();
}

fn load_css(window: &ApplicationWindow) {
    let css = r#"
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
    "#;

    let provider = gtk4::CssProvider::new();
    provider.load_from_data(css);

    gtk4::style_context_add_provider_for_display(
        &gtk4::prelude::WidgetExt::display(window),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn refresh_history(state: &Rc<AppState>) {
    // Clear existing items
    while let Some(child) = state.history_box.first_child() {
        state.history_box.remove(&child);
    }

    let recordings = state.db.get_all_recordings(100).unwrap_or_default();

    if recordings.is_empty() {
        let empty_label = gtk4::Label::builder()
            .label("No recordings yet.\nStart a recording with the Voice Input toggle.")
            .margin_top(50)
            .css_classes(vec!["dim-label"])
            .build();
        state.history_box.append(&empty_label);
    } else {
        for rec in recordings {
            let row = create_recording_row(rec, state.db.clone(), state.clone());
            state.history_box.append(&row);
        }
    }
}

fn create_recording_row(recording: Recording, db: Arc<Database>, state: Rc<AppState>) -> gtk4::Box {
    let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    main_box.add_css_class("card");
    main_box.set_margin_top(6);
    main_box.set_margin_bottom(6);
    main_box.set_margin_start(12);
    main_box.set_margin_end(12);

    let expanded = Rc::new(RefCell::new(false));

    // Header button
    let header_btn = gtk4::Button::new();
    header_btn.add_css_class("flat");

    let header_content = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    header_content.set_margin_top(12);
    header_content.set_margin_bottom(12);
    header_content.set_margin_start(12);
    header_content.set_margin_end(12);

    // Expand icon
    let expand_icon = gtk4::Image::from_icon_name("pan-end-symbolic");
    header_content.append(&expand_icon);

    // Timestamp
    let time_str = DateTime::parse_from_rfc3339(&recording.timestamp)
        .ok()
        .map(|dt| dt.format("%d.%m. %H:%M").to_string())
        .unwrap_or_else(|| "?".to_string());

    let time_label = gtk4::Label::builder()
        .label(&time_str)
        .css_classes(vec!["dim-label"])
        .width_chars(12)
        .build();
    header_content.append(&time_label);

    // Preview
    let preview_text = recording
        .llm_output
        .as_ref()
        .or(recording.whisper_output.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("");
    let preview = if preview_text.chars().count() > 60 {
        let truncated: String = preview_text.chars().take(60).collect();
        format!("{}...", truncated)
    } else {
        preview_text.to_string()
    };

    let preview_label = gtk4::Label::builder()
        .label(&preview)
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();
    header_content.append(&preview_label);

    // Status
    let status_label = if recording.user_correction.is_some() {
        gtk4::Label::builder()
            .label("corrected")
            .css_classes(vec!["success", "caption"])
            .build()
    } else if recording.success {
        gtk4::Label::builder()
            .label("")
            .css_classes(vec!["caption"])
            .build()
    } else {
        gtk4::Label::builder()
            .label("Error")
            .css_classes(vec!["error", "caption"])
            .build()
    };
    header_content.append(&status_label);

    header_btn.set_child(Some(&header_content));
    main_box.append(&header_btn);

    // Detail box (expandable)
    let detail_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    detail_box.set_margin_start(16);
    detail_box.set_margin_end(16);
    detail_box.set_margin_bottom(16);
    detail_box.set_visible(false);

    // Duration info
    if recording.total_duration_ms > 0 {
        let timing_label = gtk4::Label::builder()
            .label(&format!(
                "Duration: {}ms (Whisper: {}ms, LLM: {}ms)",
                recording.total_duration_ms,
                recording.whisper_duration_ms,
                recording.llm_duration_ms
            ))
            .css_classes(vec!["caption", "dim-label"])
            .xalign(0.0)
            .build();
        detail_box.append(&timing_label);
    }

    // Whisper output section
    let whisper_group = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    let whisper_header = gtk4::Label::builder()
        .label("Whisper (Raw Transcription)")
        .css_classes(vec!["heading"])
        .xalign(0.0)
        .build();
    whisper_group.append(&whisper_header);

    let whisper_frame = gtk4::Frame::new(None);
    let whisper_scroll = gtk4::ScrolledWindow::builder()
        .min_content_height(60)
        .max_content_height(120)
        .build();

    let whisper_view = gtk4::TextView::builder()
        .editable(false)
        .wrap_mode(gtk4::WrapMode::Word)
        .cursor_visible(false)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();
    whisper_view
        .buffer()
        .set_text(&recording.whisper_output.as_deref().unwrap_or(""));

    whisper_scroll.set_child(Some(&whisper_view));
    whisper_frame.set_child(Some(&whisper_scroll));
    whisper_group.append(&whisper_frame);
    detail_box.append(&whisper_group);

    // LLM output section
    let llm_group = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    let llm_header = gtk4::Label::builder()
        .label("LLM (Formatted)")
        .css_classes(vec!["heading"])
        .xalign(0.0)
        .build();
    llm_group.append(&llm_header);

    let llm_frame = gtk4::Frame::new(None);
    let llm_scroll = gtk4::ScrolledWindow::builder()
        .min_content_height(60)
        .max_content_height(120)
        .build();

    let llm_view = gtk4::TextView::builder()
        .editable(false)
        .wrap_mode(gtk4::WrapMode::Word)
        .cursor_visible(false)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();
    llm_view
        .buffer()
        .set_text(&recording.llm_output.as_deref().unwrap_or(""));

    llm_scroll.set_child(Some(&llm_view));
    llm_frame.set_child(Some(&llm_scroll));
    llm_group.append(&llm_frame);
    detail_box.append(&llm_group);

    // Correction section
    let corr_group = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    let corr_header = gtk4::Label::builder()
        .label("Your Correction (what you actually meant)")
        .css_classes(vec!["heading", "accent"])
        .xalign(0.0)
        .build();
    corr_group.append(&corr_header);

    let corr_frame = gtk4::Frame::new(None);
    corr_frame.add_css_class("accent-border");
    let corr_scroll = gtk4::ScrolledWindow::builder()
        .min_content_height(80)
        .max_content_height(150)
        .build();

    let corr_view = gtk4::TextView::builder()
        .editable(true)
        .wrap_mode(gtk4::WrapMode::Word)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();

    let initial_text = recording
        .user_correction
        .as_ref()
        .or(recording.llm_output.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("");
    corr_view.buffer().set_text(initial_text);

    corr_scroll.set_child(Some(&corr_view));
    corr_frame.set_child(Some(&corr_scroll));
    corr_group.append(&corr_frame);

    // Button row
    let btn_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);

    let save_btn = gtk4::Button::builder()
        .label("Save Correction")
        .css_classes(vec!["suggested-action"])
        .build();

    let delete_btn = gtk4::Button::builder()
        .label("Delete")
        .css_classes(vec!["destructive-action"])
        .build();

    let rec_id = recording.id;
    save_btn.connect_clicked(clone!(@strong db, @strong corr_view, @strong state => move |_| {
        let buffer = corr_view.buffer();
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        let correction = text.trim();

        if !correction.is_empty() {
            if let Err(e) = db.update_correction(rec_id, correction) {
                eprintln!("Failed to save correction: {}", e);
            } else {
                refresh_history(&state);
                refresh_corrections(&state);
            }
        }
    }));

    delete_btn.connect_clicked(clone!(@strong db, @strong state => move |_| {
        if let Err(e) = db.delete_recording(rec_id) {
            eprintln!("Failed to delete recording: {}", e);
        } else {
            refresh_history(&state);
            refresh_corrections(&state);
        }
    }));

    btn_row.append(&save_btn);
    btn_row.append(&delete_btn);
    corr_group.append(&btn_row);
    detail_box.append(&corr_group);

    // Error message
    if let Some(err) = &recording.error_message {
        let err_label = gtk4::Label::builder()
            .label(&format!("Error: {}", err))
            .css_classes(vec!["error"])
            .xalign(0.0)
            .wrap(true)
            .build();
        detail_box.append(&err_label);
    }

    main_box.append(&detail_box);

    // Toggle expand on header click
    header_btn.connect_clicked(clone!(@strong expanded, @strong detail_box, @strong expand_icon => move |_| {
        let is_expanded = *expanded.borrow();
        *expanded.borrow_mut() = !is_expanded;
        detail_box.set_visible(!is_expanded);
        expand_icon.set_icon_name(Some(if !is_expanded { "pan-down-symbolic" } else { "pan-end-symbolic" }));
    }));

    main_box
}

#[derive(Clone)]
struct SettingsWidgets {
    api_entry: adw::EntryRow,
    mic_entry: adw::EntryRow,
    lang_entry: adw::EntryRow,
    notif_switch: adw::SwitchRow,
    tray_switch: adw::SwitchRow,
    prompt_view: gtk4::TextView,
    reset_prompt_btn: gtk4::Button,
    save_btn: gtk4::Button,
    clear_logs_btn: gtk4::Button,
}

fn create_logs_page() -> (gtk4::Box, gtk4::TextView) {
    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    let info = gtk4::Label::builder()
        .label("Debug-Logs vom Voice Input Script")
        .css_classes(vec!["dim-label"])
        .xalign(0.0)
        .build();
    vbox.append(&info);

    let clear_btn = gtk4::Button::builder().label("Clear Logs").build();
    vbox.append(&clear_btn);

    let scrolled = gtk4::ScrolledWindow::builder().vexpand(true).build();

    let log_view = gtk4::TextView::builder()
        .editable(false)
        .monospace(true)
        .wrap_mode(gtk4::WrapMode::WordChar)
        .build();

    scrolled.set_child(Some(&log_view));
    vbox.append(&scrolled);

    (vbox, log_view)
}

fn create_settings_page(config: Rc<RefCell<EnvConfig>>) -> (gtk4::ScrolledWindow, SettingsWidgets) {
    let scrolled = gtk4::ScrolledWindow::builder().vexpand(true).build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    vbox.set_margin_top(20);
    vbox.set_margin_bottom(20);
    vbox.set_margin_start(20);
    vbox.set_margin_end(20);

    let cfg = config.borrow();

    // API Key
    let api_group = adw::PreferencesGroup::builder()
        .title("API Konfiguration")
        .build();

    let api_entry = adw::EntryRow::builder().title("Groq API Key").build();
    api_entry.set_text(cfg.get("GROQ_API_KEY").unwrap_or(""));
    api_group.add(&api_entry);
    vbox.append(&api_group);

    // Recording settings
    let rec_group = adw::PreferencesGroup::builder().title("Recording").build();

    let mic_entry = adw::EntryRow::builder()
        .title("Microphone Source (empty = default)")
        .build();
    mic_entry.set_text(cfg.get("MIC_SOURCE").unwrap_or(""));
    rec_group.add(&mic_entry);

    let lang_entry = adw::EntryRow::builder()
        .title("Language (e.g. 'de', 'en', empty = auto)")
        .build();
    lang_entry.set_text(cfg.get("LANGUAGE").unwrap_or(""));
    rec_group.add(&lang_entry);

    vbox.append(&rec_group);

    // UI settings
    let ui_group = adw::PreferencesGroup::builder().title("Interface").build();

    let notif_switch = adw::SwitchRow::builder().title("Notifications").build();
    notif_switch.set_active(cfg.get("NOTIFICATIONS").unwrap_or("true") == "true");
    ui_group.add(&notif_switch);

    let tray_switch = adw::SwitchRow::builder().title("Tray Icon").build();
    tray_switch.set_active(cfg.get("TRAY_ICON").unwrap_or("true") == "true");
    ui_group.add(&tray_switch);

    vbox.append(&ui_group);

    // System prompt
    let prompt_group = adw::PreferencesGroup::builder()
        .title("System Prompt")
        .build();
    let prompt_box = gtk4::Box::new(gtk4::Orientation::Vertical, 8);

    let prompt_info = gtk4::Label::builder()
        .label("Der System Prompt wird dem LLM gegeben, um die Formatierung zu steuern:")
        .css_classes(vec!["dim-label"])
        .xalign(0.0)
        .wrap(true)
        .build();
    prompt_box.append(&prompt_info);

    let prompt_scroll = gtk4::ScrolledWindow::builder()
        .min_content_height(150)
        .build();

    let prompt_view = gtk4::TextView::builder()
        .wrap_mode(gtk4::WrapMode::Word)
        .build();
    prompt_view.buffer().set_text(
        cfg.get("SYSTEM_PROMPT")
            .unwrap_or(EnvConfig::get_default_system_prompt()),
    );
    prompt_scroll.set_child(Some(&prompt_view));
    prompt_box.append(&prompt_scroll);

    let reset_prompt_btn = gtk4::Button::builder()
        .label("Reset to Default")
        .build();
    prompt_box.append(&reset_prompt_btn);

    prompt_group.add(&prompt_box);
    vbox.append(&prompt_group);

    // Save button
    let save_btn = gtk4::Button::builder()
        .label("Save Settings")
        .css_classes(vec!["suggested-action"])
        .build();
    vbox.append(&save_btn);

    // Clear logs button (store for later use)
    let clear_logs_btn = gtk4::Button::new();

    scrolled.set_child(Some(&vbox));

    (
        scrolled,
        SettingsWidgets {
            api_entry,
            mic_entry,
            lang_entry,
            notif_switch,
            tray_switch,
            prompt_view,
            reset_prompt_btn,
            save_btn,
            clear_logs_btn,
        },
    )
}

fn create_corrections_page(corrections_box: gtk4::Box) -> gtk4::Box {
    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    let info = gtk4::Label::builder()
        .label("<b>Saved Corrections</b>\n\nThese corrections are provided to the LLM as context\nto better understand your speech patterns.")
        .use_markup(true)
        .xalign(0.0)
        .wrap(true)
        .build();
    vbox.append(&info);

    let export_btn = gtk4::Button::builder()
        .label("Export Corrections as Prompt Context")
        .build();
    vbox.append(&export_btn);

    let scrolled = gtk4::ScrolledWindow::builder().vexpand(true).build();
    scrolled.set_child(Some(&corrections_box));
    vbox.append(&scrolled);

    vbox
}

fn refresh_corrections(state: &Rc<AppState>) {
    // Clear
    while let Some(child) = state.corrections_box.first_child() {
        state.corrections_box.remove(&child);
    }

    let corrections = state.db.get_corrections().unwrap_or_default();

    if corrections.is_empty() {
        let empty = gtk4::Label::builder()
            .label("No corrections yet.\n\nClick 'Save Correction' on a recording to add training data.")
            .css_classes(vec!["dim-label"])
            .margin_top(30)
            .build();
        state.corrections_box.append(&empty);
    } else {
        for c in corrections {
            let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
            row.set_margin_start(8);
            row.set_margin_end(8);
            row.set_margin_top(4);
            row.set_margin_bottom(4);

            let pattern = gtk4::Label::builder()
                .label(&format!("\"{}\"", c.whisper_pattern))
                .css_classes(vec!["dim-label"])
                .xalign(0.0)
                .build();
            row.append(&pattern);

            let arrow = gtk4::Label::builder().label("->").build();
            row.append(&arrow);

            let intended = gtk4::Label::builder()
                .label(&format!("\"{}\"", c.intended_text))
                .css_classes(vec!["accent"])
                .xalign(0.0)
                .build();
            row.append(&intended);

            state.corrections_box.append(&row);
        }
    }
}

fn refresh_logs(state: &Rc<AppState>) {
    let log_file = get_log_file();
    if log_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&log_file) {
            state.log_view.buffer().set_text(&content);
        }
    } else {
        state
            .log_view
            .buffer()
            .set_text("No logs yet.\n\nLogs will be created on next voice input.");
    }
}

fn start_log_watcher(state: Rc<AppState>) {
    refresh_logs(&state);

    let id = glib::timeout_add_seconds_local(2, clone!(@strong state => move || {
        let log_file = get_log_file();
        if log_file.exists() {
            if let Ok(metadata) = std::fs::metadata(&log_file) {
                if let Ok(mtime) = metadata.modified() {
                    let last_mtime = *state.last_log_mtime.borrow();
                    if mtime > last_mtime {
                        *state.last_log_mtime.borrow_mut() = mtime;
                        refresh_logs(&state);
                    }
                }
            }
        }
        glib::ControlFlow::Continue
    }));

    *state.log_watcher_id.borrow_mut() = Some(id);
}
