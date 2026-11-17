//! State shared by the UI thread and the background Minecraft host.

use crate::config::Config;
use crate::java::JavaInfo;
use crate::minecraft::{InstalledVersion, VersionSummary};
use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LogKind {
    Info,
    Warn,
    Error,
}

pub struct Shared {
    pub cfg: RwLock<Config>,
    pub versions: Mutex<Vec<VersionSummary>>,
    pub versions_loading: AtomicBool,
    pub versions_error: Mutex<String>,
    pub installed: Mutex<Vec<InstalledVersion>>,
    pub java: Mutex<Option<JavaInfo>>,
    // --------------------------------------------------------- client game
    pub game_running: AtomicBool,
    pub game_pid: AtomicU32,
    pub game: Mutex<Option<Child>>,
    pub game_log: Mutex<VecDeque<String>>,
    pub game_log_path: Mutex<Option<PathBuf>>,
    /// Missing mod dependencies the running game reported:
    /// (ids, (id, requested by) pairs, when last updated).
    pub missing_mods: Mutex<Option<(Vec<String>, Vec<(String, String)>, Instant)>>,
    // --------------------------------------------------------- hosted server
    pub mc_running: AtomicBool,
    pub mc_busy: AtomicBool,
    pub mc_child: Mutex<Option<Child>>,
    pub mc_port: AtomicU16,
    pub mc_log: Mutex<VecDeque<String>>,
    pub mc_log_path: Mutex<Option<PathBuf>>,
    pub mc_stage: Mutex<String>,
    pub mc_error: Mutex<Option<String>>,
    pub mc_started: Mutex<Option<Instant>>,
}

impl Shared {
    pub fn new(cfg: Config) -> Self {
        Self {
            cfg: RwLock::new(cfg),
            versions: Mutex::new(Vec::new()),
            versions_loading: AtomicBool::new(false),
            versions_error: Mutex::new(String::new()),
            installed: Mutex::new(Vec::new()),
            java: Mutex::new(None),
            game_running: AtomicBool::new(false),
            game_pid: AtomicU32::new(0),
            game: Mutex::new(None),
            game_log: Mutex::new(VecDeque::new()),
            game_log_path: Mutex::new(None),
            missing_mods: Mutex::new(None),
            mc_running: AtomicBool::new(false),
            mc_busy: AtomicBool::new(false),
            mc_child: Mutex::new(None),
            mc_port: AtomicU16::new(0),
            mc_log: Mutex::new(VecDeque::new()),
            mc_log_path: Mutex::new(None),
            mc_stage: Mutex::new(String::new()),
            mc_error: Mutex::new(None),
            mc_started: Mutex::new(None),
        }
    }

    pub fn cfg(&self) -> Config {
        self.cfg.read().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn set_cfg(&self, cfg: Config) {
        if let Ok(mut guard) = self.cfg.write() {
            *guard = cfg;
        }
    }

    /// Launcher journal goes to logs/launcher.log for diagnostics.
    pub fn log(&self, kind: LogKind, text: impl Into<String>) {
        let line = format!("{} [{:?}] {}
", crate::util::now_clock(), kind, text.into());
        let mut path = crate::config::data_dir();
        path.push("logs");
        let _ = std::fs::create_dir_all(&path);
        path.push("launcher.log");
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = std::io::Write::write_all(file.by_ref(), line.as_bytes());
        }
    }

    pub fn versions(&self) -> Vec<VersionSummary> {
        self.versions.lock().map(|v| v.clone()).unwrap_or_default()
    }

    pub fn versions_loading(&self) -> bool {
        self.versions_loading.load(Ordering::Relaxed)
    }

    pub fn versions_error(&self) -> String {
        self.versions_error
            .lock()
            .map(|e| e.clone())
            .unwrap_or_default()
    }

    pub fn installed(&self) -> Vec<InstalledVersion> {
        self.installed.lock().map(|v| v.clone()).unwrap_or_default()
    }

    pub fn set_installed(&self, list: Vec<InstalledVersion>) {
        if let Ok(mut guard) = self.installed.lock() {
            *guard = list;
        }
    }

    pub fn java(&self) -> Option<JavaInfo> {
        self.java.lock().ok().and_then(|j| j.clone())
    }

    pub fn set_java(&self, java: Option<JavaInfo>) {
        if let Ok(mut guard) = self.java.lock() {
            *guard = java;
        }
    }

    pub fn is_game_running(&self) -> bool {
        self.game_running.load(Ordering::Relaxed)
    }

    pub fn game_pid(&self) -> u32 {
        self.game_pid.load(Ordering::Relaxed)
    }

    pub fn set_game_pid(&self, pid: u32) {
        self.game_pid.store(pid, Ordering::Relaxed);
    }

    pub fn set_game(&self, child: Option<Child>) {
        if let Ok(mut guard) = self.game.lock() {
            *guard = child;
        }
    }

    pub fn start_game_log(&self, path: PathBuf) {
        if let Ok(mut guard) = self.game_log_path.lock() {
            *guard = Some(path);
        }
        self.clear_game_log();
    }

    pub fn clear_game_log(&self) {
        if let Ok(mut log) = self.game_log.lock() {
            log.clear();
        }
    }

    pub fn push_game(&self, line: impl Into<String>) {
        let line = line.into();
        if let Ok(mut log) = self.game_log.lock() {
            log.push_back(line.clone());
            while log.len() > 600 {
                log.pop_front();
            }
        }
        if let Ok(guard) = self.game_log_path.lock() {
            if let Some(path) = guard.as_ref() {
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    let _ = writeln!(file, "{line}");
                }
            }
        }
    }


    /// Only the last `n` lines — cheap for per-frame UI reads.
    pub fn game_log_tail(&self, n: usize) -> Vec<String> {
        self.game_log
            .lock()
            .map(|l| l.iter().rev().take(n).cloned().rev().collect())
            .unwrap_or_default()
    }


    // --------------------------------------------------------- hosted server

    pub fn is_mc_running(&self) -> bool {
        self.mc_running.load(Ordering::Relaxed)
    }

    pub fn is_mc_busy(&self) -> bool {
        self.mc_busy.load(Ordering::Relaxed)
    }

    pub fn mc_port(&self) -> u16 {
        self.mc_port.load(Ordering::Relaxed)
    }

    pub fn set_mc_port(&self, p: u16) {
        self.mc_port.store(p, Ordering::Relaxed);
    }

    pub fn mc_stage(&self) -> String {
        self.mc_stage.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn set_mc_stage(&self, stage: impl Into<String>) {
        if let Ok(mut s) = self.mc_stage.lock() {
            *s = stage.into();
        }
    }

    pub fn mc_error(&self) -> Option<String> {
        self.mc_error.lock().ok().and_then(|e| e.clone())
    }

    pub fn set_mc_error(&self, error: Option<String>) {
        if let Ok(mut guard) = self.mc_error.lock() {
            *guard = error;
        }
    }

    pub fn mc_uptime_secs(&self) -> u64 {
        self.mc_started
            .lock()
            .ok()
            .and_then(|t| t.map(|t| t.elapsed().as_secs()))
            .unwrap_or(0)
    }

    pub fn set_mc_child(&self, child: Option<Child>) {
        if let Ok(mut guard) = self.mc_child.lock() {
            *guard = child;
        }
    }

    pub fn start_mc_log(&self, path: PathBuf) {
        if let Ok(mut guard) = self.mc_log_path.lock() {
            *guard = Some(path);
        }
        self.clear_mc_log();
    }

    pub fn clear_mc_log(&self) {
        if let Ok(mut log) = self.mc_log.lock() {
            log.clear();
        }
    }

    pub fn push_mc(&self, line: impl Into<String>) {
        let line = line.into();
        if let Ok(mut log) = self.mc_log.lock() {
            log.push_back(line.clone());
            while log.len() > 600 {
                log.pop_front();
            }
        }
        if let Ok(guard) = self.mc_log_path.lock() {
            if let Some(path) = guard.as_ref() {
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    let _ = writeln!(file, "{line}");
                }
            }
        }
    }


    pub fn mc_log_tail(&self, n: usize) -> Vec<String> {
        self.mc_log
            .lock()
            .map(|l| l.iter().rev().take(n).cloned().rev().collect())
            .unwrap_or_default()
    }
}
