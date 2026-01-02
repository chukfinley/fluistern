mod config;
mod database;

use config::EnvConfig;
use database::{Database, Recording as DbRecording, Correction as DbCorrection};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::SystemTime;

slint::include_modules!();

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

fn format_timestamp(iso_str: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso_str)
        .ok()
        .map(|dt| dt.format("%d.%m. %H:%M").to_string())
        .unwrap_or_else(|| "?".to_string())
}

fn db_recording_to_slint(rec: &DbRecording) -> Recording {
    Recording {
        id: rec.id as i32,
        timestamp: format_timestamp(&rec.timestamp).into(),
        whisper_output: rec.whisper_output.clone().unwrap_or_default().into(),
        llm_output: rec.llm_output.clone().unwrap_or_default().into(),
        user_correction: rec.user_correction.clone().unwrap_or_default().into(),
        total_duration_ms: rec.total_duration_ms as i32,
        whisper_duration_ms: rec.whisper_duration_ms as i32,
        llm_duration_ms: rec.llm_duration_ms as i32,
        success: rec.success,
        error_message: rec.error_message.clone().unwrap_or_default().into(),
    }
}

fn db_correction_to_slint(corr: &DbCorrection) -> Correction {
    Correction {
        whisper_pattern: corr.whisper_pattern.clone().into(),
        intended_text: corr.intended_text.clone().into(),
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let db = Rc::new(Database::new(get_db_file()).expect("Failed to open database"));
    let config = Rc::new(std::cell::RefCell::new(EnvConfig::new(get_env_file())));

    let ui = MainWindow::new()?;

    // Initialize UI with data
    refresh_history(&ui, &db);
    refresh_corrections(&ui, &db);
    refresh_logs(&ui);
    refresh_settings(&ui, &config);

    // Setup refresh callback
    let ui_handle = ui.as_weak();
    let db_clone = db.clone();
    let config_clone = config.clone();
    ui.on_refresh(move || {
        let ui = ui_handle.unwrap();
        refresh_history(&ui, &db_clone);
        refresh_corrections(&ui, &db_clone);
        refresh_logs(&ui);
        refresh_settings(&ui, &config_clone);
    });

    // Setup save_correction callback
    let ui_handle = ui.as_weak();
    let db_clone = db.clone();
    ui.on_save_correction(move |id, text| {
        if let Err(e) = db_clone.update_correction(id as i64, &text) {
            eprintln!("Failed to save correction: {}", e);
        } else {
            let ui = ui_handle.unwrap();
            refresh_history(&ui, &db_clone);
            refresh_corrections(&ui, &db_clone);
        }
    });

    // Setup delete_recording callback
    let ui_handle = ui.as_weak();
    let db_clone = db.clone();
    ui.on_delete_recording(move |id| {
        if let Err(e) = db_clone.delete_recording(id as i64) {
            eprintln!("Failed to delete recording: {}", e);
        } else {
            let ui = ui_handle.unwrap();
            refresh_history(&ui, &db_clone);
            refresh_corrections(&ui, &db_clone);
        }
    });

    // Setup save_settings callback
    let ui_handle = ui.as_weak();
    let config_clone = config.clone();
    ui.on_save_settings(move |api_key, mic_source, language, notifications, tray_icon, system_prompt| {
        let mut cfg = config_clone.borrow_mut();
        cfg.set("GROQ_API_KEY".to_string(), api_key.to_string());
        cfg.set("MIC_SOURCE".to_string(), mic_source.to_string());
        cfg.set("LANGUAGE".to_string(), language.to_string());
        cfg.set("NOTIFICATIONS".to_string(), if notifications { "true" } else { "false" }.to_string());
        cfg.set("TRAY_ICON".to_string(), if tray_icon { "true" } else { "false" }.to_string());
        cfg.set("SYSTEM_PROMPT".to_string(), system_prompt.to_string());

        if let Err(e) = cfg.save() {
            eprintln!("Failed to save settings: {}", e);
        } else {
            println!("Settings saved!");
        }
    });

    // Setup reset_prompt callback
    let ui_handle = ui.as_weak();
    ui.on_reset_prompt(move || {
        let ui = ui_handle.unwrap();
        ui.set_system_prompt(EnvConfig::get_default_system_prompt().into());
    });

    // Setup clear_logs callback
    let ui_handle = ui.as_weak();
    ui.on_clear_logs(move || {
        let log_file = get_log_file();
        if log_file.exists() {
            let _ = std::fs::remove_file(log_file);
        }
        let ui = ui_handle.unwrap();
        ui.set_logs("".into());
    });

    // Start log watcher
    let ui_handle = ui.as_weak();
    let mut last_mtime = SystemTime::UNIX_EPOCH;
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));

            let log_file = get_log_file();
            if log_file.exists() {
                if let Ok(metadata) = std::fs::metadata(&log_file) {
                    if let Ok(mtime) = metadata.modified() {
                        if mtime > last_mtime {
                            last_mtime = mtime;
                            if let Ok(content) = std::fs::read_to_string(&log_file) {
                                let ui = ui_handle.unwrap();
                                ui.set_logs(content.into());
                            }
                        }
                    }
                }
            }
        }
    });

    ui.run()
}

fn refresh_history(ui: &MainWindow, db: &Database) {
    let recordings = db.get_all_recordings(100).unwrap_or_default();
    let slint_recordings: Vec<Recording> = recordings
        .iter()
        .map(db_recording_to_slint)
        .collect();

    let model = Rc::new(slint::VecModel::from(slint_recordings));
    ui.set_recordings(model.into());
}

fn refresh_corrections(ui: &MainWindow, db: &Database) {
    let corrections = db.get_corrections().unwrap_or_default();
    let slint_corrections: Vec<Correction> = corrections
        .iter()
        .map(db_correction_to_slint)
        .collect();

    let model = Rc::new(slint::VecModel::from(slint_corrections));
    ui.set_corrections(model.into());
}

fn refresh_logs(ui: &MainWindow) {
    let log_file = get_log_file();
    if log_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&log_file) {
            ui.set_logs(content.into());
        }
    } else {
        ui.set_logs("No logs yet.\n\nLogs will be created on next voice input.".into());
    }
}

fn refresh_settings(ui: &MainWindow, config: &std::cell::RefCell<EnvConfig>) {
    let cfg = config.borrow();
    ui.set_api_key(cfg.get("GROQ_API_KEY").unwrap_or("").into());
    ui.set_mic_source(cfg.get("MIC_SOURCE").unwrap_or("").into());
    ui.set_language(cfg.get("LANGUAGE").unwrap_or("").into());
    ui.set_notifications(cfg.get("NOTIFICATIONS").unwrap_or("true") == "true");
    ui.set_tray_icon(cfg.get("TRAY_ICON").unwrap_or("true") == "true");
    ui.set_system_prompt(cfg.get("SYSTEM_PROMPT").unwrap_or(EnvConfig::get_default_system_prompt()).into());
}
