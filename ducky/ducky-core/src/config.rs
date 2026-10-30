use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// "HH:MM" to "HH:MM". A window whose end is before its start wraps past midnight.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Window {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub active: Window,
    /// Times ducky must never speak (lunch, deep work, ...). Win over `active`.
    pub quiet: Vec<Window>,
    pub daily_token_cap: u64,
    pub daily_cost_cap_usd: f64,
    /// Minimum minutes between two questions.
    pub min_gap_minutes: u32,
    /// Target average number of questions per active hour.
    pub questions_per_hour: f64,
    /// How often the scheduler wakes `ducky-tick`.
    pub tick_minutes: u32,
    pub model: String,
    /// Speak questions and feedback aloud, and offer the microphone.
    pub voice: bool,
    /// Name of the macOS voice for `say -v`, e.g. "Ava (Premium)". Empty uses the system default.
    pub tts_voice: String,
    /// Topics (by quiz topic name) switched off: never asked, still shown in Review.
    pub disabled_topics: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            active: Window { from: "09:00".into(), to: "18:00".into() },
            quiet: vec![Window { from: "12:30".into(), to: "13:30".into() }],
            daily_token_cap: 200_000,
            daily_cost_cap_usd: 0.50,
            min_gap_minutes: 20,
            questions_per_hour: 2.0,
            tick_minutes: 10,
            model: "haiku".into(),
            voice: true,
            tts_voice: String::new(),
            disabled_topics: vec![],
        }
    }
}

/// `$DUCKY_HOME`; else, in debug builds, `ducky/.dev-home` inside the repo (gitignored) so
/// development never touches your home directory; else `~/.ducky` for release builds.
pub fn home_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("DUCKY_HOME") {
        return PathBuf::from(p);
    }
    if cfg!(debug_assertions) {
        return PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.dev-home");
    }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".ducky")
}

/// True for "HH:MM" (24 hour).
pub fn valid_time(t: &str) -> bool {
    chrono::NaiveTime::parse_from_str(t, "%H:%M").is_ok()
}

impl Config {
    /// First problem found, in words the Settings screen can show.
    pub fn validate(&self) -> Result<(), String> {
        let win = |name: &str, w: &Window| {
            if valid_time(&w.from) && valid_time(&w.to) && w.from != w.to {
                Ok(())
            } else {
                Err(format!("{name}: times must be HH:MM, and different (like 09:00 and 18:00)"))
            }
        };
        win("Active hours", &self.active)?;
        for w in &self.quiet {
            win("Quiet time", w)?;
        }
        if self.tick_minutes == 0 {
            return Err("Check interval must be at least 1 minute".into());
        }
        if !(self.questions_per_hour >= 0.0) || !(self.daily_cost_cap_usd >= 0.0) {
            return Err("Frequency and cost cap cannot be negative".into());
        }
        if self.model.trim().is_empty() {
            return Err("Model cannot be empty".into());
        }
        Ok(())
    }

    pub fn load() -> Result<Config> {
        let path = home_dir().join("config.json");
        match std::fs::read_to_string(&path) {
            Ok(s) => Ok(serde_json::from_str(&s)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self) -> Result<()> {
        let dir = home_dir();
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("config.json"), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_valid() {
        assert!(Config::default().validate().is_ok());
    }

    #[test]
    fn rejects_bad_settings() {
        let mut c = Config::default();
        c.active.from = "9am".into();
        assert!(c.validate().is_err());
        let mut c = Config::default();
        c.quiet.push(Window { from: "12:00".into(), to: "12:00".into() });
        assert!(c.validate().is_err());
        let mut c = Config::default();
        c.tick_minutes = 0;
        assert!(c.validate().is_err());
        let mut c = Config::default();
        c.questions_per_hour = f64::NAN;
        assert!(c.validate().is_err());
        let mut c = Config::default();
        c.model = " ".into();
        assert!(c.validate().is_err());
    }
}
