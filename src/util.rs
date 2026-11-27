//! OS helpers: local clock, autostart, shortcuts, single instance, native
//! window dressing. Windows uses WinAPI/registry; Linux and macOS get
//! .desktop files, a LaunchAgent and a lock file.

#[cfg(windows)]
use std::ffi::c_void;

#[cfg(windows)]
use crate::platform::hidden_command;

#[cfg(windows)]
#[repr(C)]
struct SystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetLocalTime(out: *mut SystemTime);
}

/// Local wall-clock time: (year, month, day, hour, minute, second).
#[cfg(windows)]
fn local_time() -> (u32, u32, u32, u32, u32, u32) {
    let mut st = SystemTime {
        year: 0,
        month: 0,
        day_of_week: 0,
        day: 0,
        hour: 0,
        minute: 0,
        second: 0,
        milliseconds: 0,
    };
    unsafe { GetLocalTime(&mut st) };
    (
        st.year as u32,
        st.month as u32,
        st.day as u32,
        st.hour as u32,
        st.minute as u32,
        st.second as u32,
    )
}

#[cfg(unix)]
fn local_time() -> (u32, u32, u32, u32, u32, u32) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as libc::time_t)
        .unwrap_or(0);
    // SAFETY: localtime_r only writes into the zeroed struct we pass.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&now, &mut tm) };
    (
        (tm.tm_year + 1900) as u32,
        (tm.tm_mon + 1) as u32,
        tm.tm_mday as u32,
        tm.tm_hour as u32,
        tm.tm_min as u32,
        tm.tm_sec as u32,
    )
}

pub fn now_clock() -> String {
    let (_, _, _, h, m, s) = local_time();
    format!("{:02}:{:02}:{:02}", h, m, s)
}

pub fn now_stamp() -> String {
    let (y, mo, d, h, m, s) = local_time();
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, m, s)
}

pub fn file_stamp() -> String {
    let (y, mo, d, h, m, s) = local_time();
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", y, mo, d, h, m, s)
}

/// Seconds since 1970 now (UTC).
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `2026-11-17T23:14:40.617931Z` / `…+00:00` → Unix seconds (UTC; the
/// offset is applied). Date-only strings count as midnight.
pub fn parse_iso_unix(iso: &str) -> Option<i64> {
    let iso = iso.trim();
    let num = |s: &str| s.parse::<i64>().ok();
    let (y, mo, d) = (num(iso.get(0..4)?)?, num(iso.get(5..7)?)?, num(iso.get(8..10)?)?);
    if iso.get(4..5)? != "-" || !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let (mut h, mut mi, mut s, mut offset) = (0, 0, 0, 0i64);
    if let Some(time) = iso.get(11..) {
        h = time.get(0..2).and_then(num).unwrap_or(0);
        mi = time.get(3..5).and_then(num).unwrap_or(0);
        s = time.get(6..8).and_then(num).unwrap_or(0);
        // «+03:00» / «-05:00» after the seconds (and fraction).
        if let Some(pos) = time.get(8..).and_then(|t| t.find(['+', '-'])) {
            let tz = &time[8 + pos..];
            let sign = if tz.starts_with('-') { -1 } else { 1 };
            let oh = tz.get(1..3).and_then(num).unwrap_or(0);
            let om = tz.get(4..6).and_then(num).unwrap_or(0);
            offset = sign * (oh * 3600 + om * 60);
        }
    }
    // Days from civil date (Howard Hinnant's algorithm).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + s - offset)
}

/// «17 нояб. 2026» from an ISO 8601 date; empty if unreadable.
pub fn ru_date(iso: &str) -> String {
    const MONTHS: [&str; 12] = [
        "янв.", "февр.", "марта", "апр.", "мая", "июня", "июля", "авг.", "сент.", "окт.", "нояб.", "дек.",
    ];
    if parse_iso_unix(iso).is_none() {
        return String::new();
    }
    let num = |a: usize, b: usize| iso.get(a..b).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    let (y, m, d) = (num(0, 4), num(5, 7), num(8, 10));
    format!("{d} {} {y}", MONTHS[m.clamp(1, 12) - 1])
}

pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["Б", "КБ", "МБ", "ГБ"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    // Russian decimal comma: «1,6 ГБ».
    if i == 0 {
        format!("{} {}", n, UNITS[0])
    } else if v >= 100.0 {
        format!("{:.0} {}", v, UNITS[i])
    } else {
        format!("{:.1} {}", v, UNITS[i]).replace('.', ",")
    }
}

pub fn human_uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{} ч {} мин", h, m)
    } else if m > 0 {
        format!("{} мин {} с", m, s)
    } else {
        format!("{} с", s)
    }
}

#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const RUN_VALUE: &str = crate::config::APP_TITLE;

#[cfg(windows)]
pub fn autostart_enabled() -> bool {
    crate::win::reg_has_value(RUN_KEY, RUN_VALUE)
}

#[cfg(unix)]
pub fn autostart_enabled() -> bool {
    unix::autostart_file().map(|p| p.is_file()).unwrap_or(false)
}

#[cfg(unix)]
pub fn set_autostart(enable: bool) -> bool {
    unix::set_autostart(enable)
}

#[cfg(unix)]
pub fn migrate_legacy_names() {
    // The old «LLM Launcher Madrid» only ever shipped for Windows.
}

#[cfg(unix)]
pub fn create_desktop_shortcut() -> bool {
    unix::create_shortcut()
}

#[cfg(unix)]
pub fn shortcut_exists() -> bool {
    unix::shortcut_paths().first().map(|p| p.symlink_metadata().is_ok()).unwrap_or(false)
}

#[cfg(windows)]
pub fn set_autostart(enable: bool) -> bool {
    if enable {
        let exe = std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        crate::win::reg_set_string(RUN_KEY, RUN_VALUE, &format!("\"{exe}\""))
    } else {
        crate::win::reg_delete_value(RUN_KEY, RUN_VALUE)
    }
}

/// Pre-0.7 leftovers under the old name: autostart value and desktop /
/// Start-menu shortcuts. The autostart entry is re-created under the new
/// name, the old shortcut is replaced by the regular first-run shortcut.
#[cfg(windows)]
pub fn migrate_legacy_names() {
    let legacy = crate::config::LEGACY_NAME;
    if crate::win::reg_has_value(RUN_KEY, legacy) {
        crate::win::reg_delete_value(RUN_KEY, legacy);
        set_autostart(true);
    }
    if let Some(desktop) = dirs::desktop_dir() {
        let _ = std::fs::remove_file(desktop.join(format!("{legacy}.lnk")));
    }
    if let Some(roaming) = dirs::data_dir() {
        let start = roaming.join(r"Microsoft\Windows\Start Menu\Programs").join(legacy);
        let _ = std::fs::remove_dir_all(start);
    }
}

pub fn open_path(path: &std::path::Path) {
    let _ = open::that_detached(path);
}

/// Desktop shortcut named after the launcher; the icon comes from the exe.
/// The Desktop may live in OneDrive — `dirs` asks the shell where it is.
#[cfg(windows)]
pub fn create_desktop_shortcut() -> bool {
    let (Ok(exe), Some(desktop)) = (std::env::current_exe(), dirs::desktop_dir()) else {
        return false;
    };
    crate::win::create_shortcut(&desktop.join("Sad Launcher.lnk"), &exe, "Minecraft Madrid · Forge")
}

/// True when the Desktop .lnk already exists (OneDrive-aware).
#[cfg(windows)]
pub fn shortcut_exists() -> bool {
    match dirs::desktop_dir() {
        Some(desktop) => desktop.join("Sad Launcher.lnk").exists(),
        None => false,
    }
}

// ---------------------------------------------------------------- disk hygiene

/// Size of a directory tree, best effort.
pub fn dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

/// Removes re-downloadable caches: Forge installers, temp parts, old logs.
/// Game files stay — anything deleted here re-downloads from Mojang/Forge.
pub fn clear_cache(data: &std::path::Path) -> u64 {    let mut freed = 0u64;
    let mut remove_all = |dir: &std::path::Path, pred: &dyn Fn(&std::path::Path) -> bool| {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if pred(&p) {
                    if p.is_dir() {
                        freed += dir_size(&p);
                        let _ = std::fs::remove_dir_all(&p);
                    } else if let Ok(meta) = entry.metadata() {
                        freed += meta.len();
                        let _ = std::fs::remove_file(&p);
                    }
                }
            }
        }
    };
    let installer_dir = data.join("meta").join("forge");
    remove_all(&installer_dir, &|_| true);
    // Leftover modpack archives from interrupted installs.
    remove_all(&data.join("meta").join("packs"), &|_| true);
    let logs = data.join("logs");
    let week_ago = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .saturating_sub(7 * 24 * 3600);
    remove_all(&logs, &|p| {
        p.extension().map(|e| e == "log").unwrap_or(false)
            && p.metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() < week_ago)
                .unwrap_or(false)
    });
    for stale in ["game/versions", "server"] {
        remove_all(&data.join(stale), &|p| {
            p.extension().map(|e| e == "part").unwrap_or(false)
        });
    }
    freed
}

/// Keeps the launcher's own data small, quietly, at start-up:
/// picture cache ≤ 40 MB (oldest go first), the last 12 game and server
/// logs, no leftovers of removed features or interrupted downloads.
pub fn prune_data(data: &std::path::Path) {
    fn files(dir: &std::path::Path) -> Vec<(std::path::PathBuf, u64, std::time::SystemTime)> {
        std::fs::read_dir(dir)
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        let m = e.metadata().ok()?;
                        m.is_file().then(|| (e.path(), m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    // Pictures: newest first, drop the tail past 40 MB.
    let mut icons = files(&data.join("meta").join("icons"));
    icons.sort_by(|a, b| b.2.cmp(&a.2));
    let mut kept = 0u64;
    for (path, len, _) in icons {
        kept += len;
        if kept > 40 * 1024 * 1024 {
            let _ = std::fs::remove_file(path);
        }
    }
    // Logs: the latest dozen of each kind is plenty to read a crash.
    let mut logs = files(&data.join("logs"));
    logs.sort_by(|a, b| b.2.cmp(&a.2));
    for prefix in ["game-", "server-", "jvm-crash-"] {
        for (path, _, _) in logs
            .iter()
            .filter(|(p, _, _)| p.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with(prefix)).unwrap_or(false))
            .skip(12)
        {
            let _ = std::fs::remove_file(path);
        }
    }
    for (path, _, _) in &logs {
        // Logs of the removed stevenarella edition.
        if path.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with("rustmc-")).unwrap_or(false) {
            let _ = std::fs::remove_file(path);
        }
    }
    for (path, _, _) in files(&data.join("meta").join("packs")) {
        let _ = std::fs::remove_file(path);
    }
}

// ---------------------------------------------------------------- single instance

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn CreateMutexW(attr: *mut c_void, initial: i32, name: *const u16) -> *mut c_void;
    fn GetLastError() -> u32;
}

#[cfg(windows)]
const ERROR_ALREADY_EXISTS: u32 = 183;

/// Held for the whole run: a mutex handle on Windows, a locked file elsewhere.
#[cfg(windows)]
pub struct InstanceLock(#[allow(dead_code)] *mut c_void);
#[cfg(unix)]
pub struct InstanceLock(#[allow(dead_code)] Option<std::fs::File>);

/// Returns a leaked mutex handle while we are the first instance,
/// or None when another copy already runs.
#[cfg(windows)]
pub fn lock_single_instance() -> Option<InstanceLock> {
    let name: Vec<u16> = "LLMLauncherMadrid-Instance\0"
        .encode_utf16()
        .collect();
    let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) };
    if handle.is_null() {
        return Some(InstanceLock(std::ptr::null_mut())); // can't tell — allow the window anyway
    }
    let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if exists {
        None
    } else {
        Some(InstanceLock(handle)) // intentionally never released: it pins the instance
    }
}

/// Exclusive lock on `<data>/instance.lock`; the OS drops it when the
/// process ends, even after a crash, so a stale file never blocks a start.
#[cfg(unix)]
pub fn lock_single_instance() -> Option<InstanceLock> {
    let path = crate::config::data_dir().join("instance.lock");
    let file = match std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&path) {
        Ok(f) => f,
        Err(_) => return Some(InstanceLock(None)), // can't tell — allow the window anyway
    };
    match file.try_lock() {
        Ok(()) => Some(InstanceLock(Some(file))),
        Err(std::fs::TryLockError::WouldBlock) => None,
        Err(_) => Some(InstanceLock(None)),
    }
}

/// Another copy owns the window; on Linux/macOS there is no portable way to
/// raise a foreign window, so the second copy just says so and quits.
#[cfg(unix)]
pub fn focus_window(_title: &str) {
    eprintln!("{} уже запущен", crate::config::APP_TITLE);
}

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
    fn ShowWindow(hwnd: *mut c_void, cmd: i32) -> i32;
    fn IsIconic(hwnd: *mut c_void) -> i32;
}

#[cfg(windows)]
const SW_RESTORE: i32 = 9;

#[cfg(windows)]
pub fn focus_window(title: &str) {
    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) };
    if hwnd.is_null() {
        return;
    }
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd);
    }
}

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(hwnd: *mut c_void, text: *const u16, caption: *const u16, flags: u32) -> i32;
    fn OpenClipboard(hwnd: *mut c_void) -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(format: u32, mem: *mut c_void) -> *mut c_void;
    fn CloseClipboard() -> i32;
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
    fn GlobalLock(mem: *mut c_void) -> *mut c_void;
    fn GlobalUnlock(mem: *mut c_void) -> i32;
    fn GlobalFree(mem: *mut c_void) -> *mut c_void;
}

/// Copies `text` to the clipboard reliably (egui output + native Win32 fallback with retries).
pub fn copy_text(ctx: &egui::Context, text: &str) {
    ctx.copy_text(text.to_string());
    #[cfg(windows)]
    copy_win32(text);
}

#[cfg(windows)]
fn copy_win32(text: &str) {
    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 0x0002;
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    let byte_len = wide.len() * std::mem::size_of::<u16>();

    unsafe {
        let mut opened = false;
        for _ in 0..6 {
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        if !opened {
            return;
        }
        EmptyClipboard();
        let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len);
        if !hmem.is_null() {
            let ptr = GlobalLock(hmem) as *mut u16;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                GlobalUnlock(hmem);
                if SetClipboardData(CF_UNICODETEXT, hmem).is_null() {
                    GlobalFree(hmem);
                }
            } else {
                GlobalFree(hmem);
            }
        }
        CloseClipboard();
    }
}

#[cfg(unix)]
pub fn message_box(text: &str) {
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(crate::config::APP_TITLE)
        .set_description(text)
        .show();
}

#[cfg(windows)]
pub fn message_box(text: &str) {
    let to_wide = |s: &str| -> Vec<u16> {
        let mut v: Vec<u16> = s.encode_utf16().collect();
        v.push(0);
        v
    };
    let body = to_wide(text);
    let title = to_wide(crate::config::APP_TITLE);
    unsafe {
        MessageBoxW(std::ptr::null_mut(), body.as_ptr(), title.as_ptr(), 0x10);
    }
}

// ---------------------------------------------------------------- native frame

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn FindWindowW(class: *const u16, title: *const u16) -> *mut c_void;
    fn SetWindowPos(
        hwnd: *mut c_void,
        after: *mut c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
}

#[cfg(windows)]
#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: *mut c_void,
        attr: u32,
        value: *const c_void,
        size: u32,
    ) -> i32;
}

#[cfg(windows)]
const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
#[cfg(windows)]
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
#[cfg(windows)]
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;

/// Dark frame, no backdrop, round corners — DWM only. Linux/macOS
/// compositors already honour the transparent frameless window.
#[cfg(unix)]
pub fn dress_window(_title: &str) {}

#[cfg(windows)]
pub fn dress_window(title: &str) {
    let mut wide: Vec<u16> = title.encode_utf16().collect();
    wide.push(0);
    unsafe {
        // The window appears a few frames after the event loop starts.
        let mut tries = 0;
        let mut hwnd = std::ptr::null_mut();
        while tries < 120 {
            hwnd = FindWindowW(std::ptr::null(), wide.as_ptr());
            if !hwnd.is_null() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
            tries += 1;
        }
        if hwnd.is_null() {
            return;
        }

        let dark: i32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const i32 as *const c_void,
            4,
        );
        // No system backdrop: Mica/Acrylic are nearly opaque, so the desktop
        // would never show through. A raw transparent surface lets the slab
        // alpha do the work down to 20%.
        let backdrop: i32 = 1; // DWMSBT_NONE
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &backdrop as *const i32 as *const c_void,
            4,
        );
        let round: i32 = 2;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &round as *const i32 as *const c_void,
            4,
        );
        // Keep the acrylic surface from being dragged behind other windows oddly.
        let _ = SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 0, 0, 0x0002 | 0x0001 | 0x0004);
    }
}

// ---------------------------------------------------------------- full removal

/// Wipes every trace of the launcher: data dir, shortcut, autostart, exe.
/// On Linux/macOS a running binary can be unlinked directly; only the
/// binary goes — its folder may be a shared one like ~/.local/bin.
#[cfg(unix)]
pub fn uninstall_everything(shared: &std::sync::Arc<crate::shared::Shared>) {
    crate::server::shutdown(shared);
    let _ = crate::launch::stop(shared);

    set_autostart(false);
    for p in unix::shortcut_paths() {
        let _ = std::fs::remove_file(p);
    }
    let _ = std::fs::remove_dir_all(crate::config::data_dir());
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(exe);
    }
    std::process::exit(0);
}

#[cfg(windows)]
fn is_dedicated_install_dir(dir: &std::path::Path) -> bool {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return false;
    };
    let expected = std::path::PathBuf::from(local)
        .join("Programs")
        .join(crate::config::APP_DIR);
    let norm = |p: &std::path::Path| -> String {
        p.to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    };
    norm(dir) == norm(&expected)
}

#[cfg(windows)]
pub fn uninstall_everything(shared: &std::sync::Arc<crate::shared::Shared>) {
    crate::server::shutdown(shared);
    let _ = crate::launch::stop(shared);

    set_autostart(false);
    if let Some(desktop) = dirs::desktop_dir() {
        let _ = std::fs::remove_file(desktop.join("Sad Launcher.lnk"));
    }
    let data = crate::config::data_dir();
    let _ = std::fs::remove_dir_all(&data);

    // Remove ourselves: copy a tiny deleter to temp, exit, it cleans up.
    // Pass paths through env vars so special chars never inject into batch,
    // and only remove the folder itself if it is our dedicated install dir.
    let self_exe = std::env::current_exe().unwrap_or_default();
    let dedicated_dir = self_exe
        .parent()
        .filter(|p| is_dedicated_install_dir(p))
        .map(|p| p.to_path_buf());
    let tmp = std::env::var_os("TEMP")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let rd_line = if dedicated_dir.is_some() {
        "if defined SAD_UNINSTALL_DIR rd /s /q \"%SAD_UNINSTALL_DIR%\" 2>nul\r\n"
    } else {
        ""
    };
    let script = format!(
        "@echo off\r\n:loop\r\ndel /f /q \"%SAD_UNINSTALL_EXE%\" 2>nul\r\nif exist \"%SAD_UNINSTALL_EXE%\" (ping 127.0.0.1 -n 2 >nul & goto loop)\r\n{rd_line}del \"%~f0\" 2>nul\r\n"
    );
    let bat = tmp.join("madrid-uninstall.bat");
    let _ = std::fs::write(&bat, script);
    let mut cmd = hidden_command("cmd");
    cmd.env("SAD_UNINSTALL_EXE", &self_exe);
    if let Some(dir) = dedicated_dir {
        cmd.env("SAD_UNINSTALL_DIR", dir);
    } else {
        cmd.env_remove("SAD_UNINSTALL_DIR");
    }
    cmd.args(["/c", &bat.display().to_string()]).spawn().ok();
    std::process::exit(0);
}


/// Nickname rules: 3..16 chars, letters/digits/underscore only.
pub fn sanitize_nickname(raw: &str) -> String {
    let filtered: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .take(16)
        .collect();
    filtered
}

pub fn nickname_valid(raw: &str) -> bool {
    let len = sanitize_nickname(raw).chars().count();
    (3..=16).contains(&len)
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn SetProcessWorkingSetSize(process: *mut c_void, min: usize, max: usize) -> i32;
}

/// Hands the launcher's idle pages back to Windows (what Task Manager shows
/// as «Память»). Pages come back on their own when touched again — called
/// when the window is hidden or the game takes over.
#[cfg(windows)]
pub fn trim_memory() {
    unsafe {
        SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

/// glibc keeps freed heap pages; give them back to the system.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub fn trim_memory() {
    // SAFETY: malloc_trim has no preconditions.
    unsafe {
        libc::malloc_trim(0);
    }
}

/// macOS / other libc: the allocator returns pages by itself.
#[cfg(all(unix, not(all(target_os = "linux", target_env = "gnu"))))]
pub fn trim_memory() {}

/// GPU choice per app lives in the Windows registry; Linux (PRIME) and
/// macOS pick the GPU themselves.
#[cfg(unix)]
pub fn prefer_dedicated_gpu(_exe: &str) {}

/// Asks Windows to run `exe` on the high-performance GPU (Settings →
/// Display → Graphics, per app, current user only). On laptops with two
/// GPUs Java otherwise lands on the integrated one and the game crawls.
#[cfg(windows)]
pub fn prefer_dedicated_gpu(exe: &str) {
    if exe.trim().is_empty() {
        return;
    }
    crate::win::reg_set_string(
        r"Software\Microsoft\DirectX\UserGpuPreferences",
        exe,
        "GpuPreference=2;",
    );
}

// ---------------------------------------------------------------- Linux / macOS

/// Autostart and shortcuts without a registry: XDG .desktop files on Linux,
/// a LaunchAgent and a Desktop alias (symlink) on macOS.
#[cfg(unix)]
mod unix {
    use std::path::{Path, PathBuf};

    const APP: &str = crate::config::APP_TITLE;
    #[cfg(target_os = "macos")]
    const AGENT_ID: &str = "com.sadlauncher.launcher";

    fn exe() -> Option<PathBuf> {
        std::env::current_exe().ok()
    }

    /// What a shortcut should open: the `.app` bundle when we run from one.
    #[cfg(target_os = "macos")]
    fn launch_target() -> Option<PathBuf> {
        let exe = exe()?;
        let bundle = exe
            .ancestors()
            .find(|p| p.extension().map(|e| e == "app").unwrap_or(false))
            .map(Path::to_path_buf);
        Some(bundle.unwrap_or(exe))
    }

    /// Desktop Entry `Exec=` quoting: wrap in quotes, escape `"` `` ` `` `$` `\`.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub(super) fn desktop_quote(path: &Path) -> String {
        let mut out = String::from("\"");
        for c in path.display().to_string().chars() {
            if matches!(c, '"' | '`' | '$' | '\\') {
                out.push('\\');
            }
            out.push(c);
        }
        out.push('"');
        out
    }

    /// The launcher icon, written once next to the data so menus can show it.
    #[cfg(not(target_os = "macos"))]
    fn icon_file() -> Option<PathBuf> {
        let path = crate::config::data_dir().join("meta").join("sad-launcher.png");
        if !path.is_file() {
            std::fs::create_dir_all(path.parent()?).ok()?;
            std::fs::write(&path, include_bytes!("../assets/icon.png")).ok()?;
        }
        Some(path)
    }

    #[cfg(not(target_os = "macos"))]
    fn desktop_entry(autostart: bool) -> Option<String> {
        let exe = exe()?;
        let icon = icon_file().map(|p| p.display().to_string()).unwrap_or_default();
        let mut text = format!(
            "[Desktop Entry]\nType=Application\nName={APP}\nComment=Minecraft · Forge\n\
             Exec={}\nIcon={icon}\nTerminal=false\nCategories=Game;\n",
            desktop_quote(&exe)
        );
        if autostart {
            text.push_str("X-GNOME-Autostart-enabled=true\n");
        }
        Some(text)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn autostart_file() -> Option<PathBuf> {
        Some(dirs::config_dir()?.join("autostart").join("sad-launcher.desktop"))
    }

    #[cfg(target_os = "macos")]
    pub fn autostart_file() -> Option<PathBuf> {
        Some(dirs::home_dir()?.join("Library").join("LaunchAgents").join(format!("{AGENT_ID}.plist")))
    }

    #[cfg(target_os = "macos")]
    fn xml_escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }

    pub fn set_autostart(enable: bool) -> bool {
        let Some(path) = autostart_file() else {
            return false;
        };
        if !enable {
            return match std::fs::remove_file(&path) {
                Ok(()) => true,
                Err(e) => e.kind() == std::io::ErrorKind::NotFound,
            };
        }
        #[cfg(not(target_os = "macos"))]
        let text = desktop_entry(true);
        #[cfg(target_os = "macos")]
        let text = exe().map(|exe| {
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
                 <plist version=\"1.0\"><dict>\n\
                 <key>Label</key><string>{AGENT_ID}</string>\n\
                 <key>ProgramArguments</key><array><string>{}</string></array>\n\
                 <key>RunAtLoad</key><true/>\n\
                 </dict></plist>\n",
                xml_escape(&exe.display().to_string())
            )
        });
        let Some(text) = text else {
            return false;
        };
        path.parent().map(std::fs::create_dir_all);
        std::fs::write(&path, text).is_ok()
    }

    /// Shortcut locations, the one `shortcut_exists` checks first.
    /// Linux: the app menu entry (always there) and a Desktop launcher.
    #[cfg(not(target_os = "macos"))]
    pub fn shortcut_paths() -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Some(data) = dirs::data_dir() {
            out.push(data.join("applications").join("sad-launcher.desktop"));
        }
        if let Some(desktop) = dirs::desktop_dir() {
            out.push(desktop.join("sad-launcher.desktop"));
        }
        out
    }

    #[cfg(target_os = "macos")]
    pub fn shortcut_paths() -> Vec<PathBuf> {
        dirs::desktop_dir().map(|d| vec![d.join(APP)]).unwrap_or_default()
    }

    #[cfg(not(target_os = "macos"))]
    pub fn create_shortcut() -> bool {
        let Some(text) = desktop_entry(false) else {
            return false;
        };
        let mut any = false;
        for path in shortcut_paths() {
            path.parent().map(std::fs::create_dir_all);
            if std::fs::write(&path, &text).is_ok() {
                // File managers launch only executable .desktop files.
                crate::platform::make_executable(&path);
                any = true;
            }
        }
        any
    }

    #[cfg(target_os = "macos")]
    pub fn create_shortcut() -> bool {
        let (Some(target), Some(link)) = (launch_target(), shortcut_paths().into_iter().next()) else {
            return false;
        };
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(target, link).is_ok()
    }
}

#[cfg(all(test, unix))]
mod unix_tests {
    #[test]
    fn desktop_exec_is_quoted() {
        let p = std::path::Path::new("/home/a b/$x\"y");
        assert_eq!(super::unix::desktop_quote(p), "\"/home/a b/\\$x\\\"y\"");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn cyrillic_nicknames_count_chars_not_bytes() {
        assert!(!super::nickname_valid("Ян"));
        assert!(super::nickname_valid("Абв"));
        assert!(super::nickname_valid("Александр"));
        assert!(super::nickname_valid("ШестнадцатьСимво"));
        assert!(!super::nickname_valid("!!!"));
    }
}
