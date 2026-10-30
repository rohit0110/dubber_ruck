//! Woken by launchd every few minutes. Decides whether ducky should speak; if not, exits at once.
//! Usage: ducky-tick [--dry-run] | install | uninstall | status
use anyhow::{Context, Result};
use chrono::{Local, NaiveDateTime};
use ducky_core::budget;
use ducky_core::config::{home_dir, Config};
use ducky_core::schedule::{decide, Decision, Inputs};
use std::path::PathBuf;
use std::process::Command;

const LABEL: &str = "com.ducky.tick";
/// A lock older than this is treated as left behind by a crashed quiz window.
const STALE_LOCK_MINUTES: u64 = 30;

fn lock_path() -> PathBuf {
    home_dir().join("quiz.lock")
}

fn state_path() -> PathBuf {
    home_dir().join("last_asked")
}

fn quiz_open() -> bool {
    std::fs::metadata(lock_path())
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .map(|age| age.as_secs() < STALE_LOCK_MINUTES * 60)
        .unwrap_or(false)
}

fn last_asked() -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(std::fs::read_to_string(state_path()).ok()?.trim(), "%Y-%m-%dT%H:%M:%S").ok()
}

fn plist(exe: &str, tick_minutes: u32, log: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key><array><string>{exe}</string></array>
  <key>StartInterval</key><integer>{}</integer>
  <key>RunAtLoad</key><false/>
  <key>ProcessType</key><string>Background</string>
  <key>LowPriorityIO</key><true/>
  <key>StandardErrorPath</key><string>{log}</string>
</dict>
</plist>
"#,
        tick_minutes * 60
    )
}

fn plist_path() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Library/LaunchAgents").join(format!("{LABEL}.plist"))
}

fn uid() -> Result<String> {
    let out = Command::new("id").arg("-u").output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn install(cfg: &Config) -> Result<()> {
    std::fs::create_dir_all(home_dir())?;
    let exe = std::env::current_exe()?.canonicalize()?;
    let log = home_dir().join("tick.log");
    let path = plist_path();
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, plist(&exe.to_string_lossy(), cfg.tick_minutes, &log.to_string_lossy()))?;
    let target = format!("gui/{}", uid()?);
    // Replace any previous version, then load.
    let _ = Command::new("launchctl").args(["bootout", &format!("{target}/{LABEL}")]).output();
    let st = Command::new("launchctl").arg("bootstrap").arg(&target).arg(&path).status()?;
    anyhow::ensure!(st.success(), "launchctl bootstrap failed");
    println!("installed: wakes every {} min ({})", cfg.tick_minutes, path.display());
    Ok(())
}

fn uninstall() -> Result<()> {
    let _ = Command::new("launchctl").args(["bootout", &format!("gui/{}/{LABEL}", uid()?)]).output();
    match std::fs::remove_file(plist_path()) {
        Ok(()) => println!("uninstalled"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => println!("was not installed"),
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

fn tick(cfg: &Config, dry_run: bool) -> Result<()> {
    let now = Local::now().naive_local();
    let inputs = Inputs {
        now,
        used_today: budget::used_on(now.date()),
        last_asked: last_asked(),
        quiz_open: quiz_open(),
        calendar_busy: false, // calendar gate arrives with the ICS reader
        roll: rand::random::<f64>(),
    };
    let d = decide(cfg, &inputs);
    if dry_run {
        println!("{d:?} (used today: {:?})", inputs.used_today);
        return Ok(());
    }
    if d != Decision::Ask {
        return Ok(());
    }
    std::fs::create_dir_all(home_dir())?;
    std::fs::write(state_path(), now.format("%Y-%m-%dT%H:%M:%S").to_string())?;
    // The quiz window ships next to this binary once it exists.
    let ui = std::env::current_exe()?.with_file_name("ducky-ui");
    if ui.exists() {
        Command::new(ui).arg("--ask").spawn().context("could not start ducky-ui")?;
    } else {
        eprintln!("would ask a question now, but ducky-ui is not built yet");
    }
    Ok(())
}

fn main() -> Result<()> {
    let cfg = Config::load()?;
    match std::env::args().nth(1).as_deref() {
        Some("install") => install(&cfg),
        Some("uninstall") => uninstall(),
        Some("status") => {
            println!("installed: {}\nquiz open: {}\nlast asked: {:?}", plist_path().exists(), quiz_open(), last_asked());
            tick(&cfg, true)
        }
        Some("--dry-run") => tick(&cfg, true),
        _ => tick(&cfg, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_has_interval_and_path() {
        let p = plist("/bin/ducky-tick", 10, "/tmp/t.log");
        assert!(p.contains("<integer>600</integer>"));
        assert!(p.contains("<string>/bin/ducky-tick</string>"));
        assert!(p.contains(LABEL));
    }
}
