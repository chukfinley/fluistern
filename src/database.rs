use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result, Row};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Recording {
    pub id: i64,
    pub timestamp: String,
    pub whisper_output: Option<String>,
    pub llm_output: Option<String>,
    pub user_correction: Option<String>,
    pub audio_duration_ms: i64,
    pub whisper_duration_ms: i64,
    pub llm_duration_ms: i64,
    pub total_duration_ms: i64,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Correction {
    pub id: i64,
    pub whisper_pattern: String,
    pub intended_text: String,
    pub created_at: String,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Database { conn };
        db.create_tables()?;
        Ok(db)
    }

    fn create_tables(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS recordings (
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
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS corrections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                whisper_pattern TEXT NOT NULL,
                intended_text TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        Ok(())
    }

    pub fn get_all_recordings(&self, limit: usize) -> Result<Vec<Recording>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM recordings ORDER BY timestamp DESC LIMIT ?",
        )?;

        let recordings = stmt
            .query_map(params![limit], |row| {
                Ok(Recording {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    whisper_output: row.get(2)?,
                    llm_output: row.get(3)?,
                    user_correction: row.get(4)?,
                    audio_duration_ms: row.get(5).unwrap_or(0),
                    whisper_duration_ms: row.get(6).unwrap_or(0),
                    llm_duration_ms: row.get(7).unwrap_or(0),
                    total_duration_ms: row.get(8).unwrap_or(0),
                    success: row.get::<_, i64>(9).unwrap_or(1) != 0,
                    error_message: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(recordings)
    }

    pub fn get_recording(&self, id: i64) -> Result<Option<Recording>> {
        let mut stmt = self.conn.prepare("SELECT * FROM recordings WHERE id = ?")?;

        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(Recording {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                whisper_output: row.get(2)?,
                llm_output: row.get(3)?,
                user_correction: row.get(4)?,
                audio_duration_ms: row.get(5).unwrap_or(0),
                whisper_duration_ms: row.get(6).unwrap_or(0),
                llm_duration_ms: row.get(7).unwrap_or(0),
                total_duration_ms: row.get(8).unwrap_or(0),
                success: row.get::<_, i64>(9).unwrap_or(1) != 0,
                error_message: row.get(10)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn update_correction(&self, recording_id: i64, user_correction: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE recordings SET user_correction = ? WHERE id = ?",
            params![user_correction, recording_id],
        )?;

        // Also save as a correction pattern
        if let Some(recording) = self.get_recording(recording_id)? {
            if let Some(whisper_output) = recording.whisper_output {
                let now = Utc::now().to_rfc3339();
                self.conn.execute(
                    "INSERT INTO corrections (whisper_pattern, intended_text, created_at)
                     VALUES (?, ?, ?)",
                    params![whisper_output, user_correction, now],
                )?;
            }
        }

        Ok(())
    }

    pub fn delete_recording(&self, recording_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM recordings WHERE id = ?",
            params![recording_id],
        )?;
        Ok(())
    }

    pub fn get_corrections(&self) -> Result<Vec<Correction>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM corrections ORDER BY created_at DESC")?;

        let corrections = stmt
            .query_map([], |row| {
                Ok(Correction {
                    id: row.get(0)?,
                    whisper_pattern: row.get(1)?,
                    intended_text: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(corrections)
    }

    pub fn export_corrections_for_prompt(&self) -> Result<String> {
        let corrections = self.get_corrections()?;

        if corrections.is_empty() {
            return Ok(String::new());
        }

        let mut lines = vec![
            "\n\nUser correction patterns (use these to better understand what the user means):"
                .to_string(),
        ];

        for c in corrections.iter().take(20) {
            lines.push(format!(
                "- When transcribed as \"{}\", the user meant: \"{}\"",
                c.whisper_pattern, c.intended_text
            ));
        }

        Ok(lines.join("\n"))
    }
}
