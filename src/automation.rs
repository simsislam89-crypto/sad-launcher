//! Автокликер (строго внутри окна Minecraft) и защита от АФК.
//! На Windows: клик — настоящий, пока игра держит курсор внутри своего окна,
//! иначе сообщением прямо в окно (`PostMessageW`); F3+T — сообщениями, когда
//! игра в фокусе; бинды — `GetAsyncKeyState`, окно — по PID/классу;
//! на других ОС собирается с безопасной заглушкой, чтобы не ломать кросс-сборку.

use crate::effects::EffectsConfig;
use crate::shared::Shared;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindTarget {
    Autoclicker,
    AntiAfk,
}

#[derive(Clone, Debug, PartialEq, Eq)]
// Key capture is Win32-only (GetAsyncKeyState); elsewhere nothing builds these.
#[cfg_attr(not(windows), allow(dead_code))]
pub enum BindCapture {
    Set(String),
    Clear,
    Cancel,
}

#[derive(Clone, Debug)]
pub struct AutomationSnapshot {
    pub mc_running: bool,
    pub mc_focused: bool,
    pub anti_afk: bool,
    pub autoclicker: bool,
    pub afk_ticks: u64,
    pub click_count: u64,
    /// Effects changed while the player was away: F3+T waits for the game
    /// to be played again (focused, cursor captured).
    pub reload_pending: bool,
    /// How many F3+T reloads were sent (the app reports each one).
    pub reload_seq: u64,
    pub next_afk_in: Option<u32>,
    pub mc_title: String,
    pub toggle_seq: u64,
}

impl Default for AutomationSnapshot {
    fn default() -> Self {
        Self {
            mc_running: false,
            mc_focused: false,
            anti_afk: false,
            autoclicker: false,
            afk_ticks: 0,
            click_count: 0,
            reload_pending: false,
            reload_seq: 0,
            next_afk_in: None,
            mc_title: String::new(),
            toggle_seq: 0,
        }
    }
}

/// Keys of «reload resource packs» from the game's `options.txt`: the
/// debug modifier (F3) and the reload key (T), rebindable since 1.21.9.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReloadKeys {
    pub modifier: u16,
    pub key: u16,
}

impl Default for ReloadKeys {
    fn default() -> Self {
        Self { modifier: 0x72, key: 0x54 }
    }
}

/// `key.keyboard.f3` → VK_F3; letters, digits and F1–F24 only.
fn mc_key_vk(name: &str) -> Option<u16> {
    let k = name.trim().strip_prefix("key.keyboard.")?;
    if let Some(n) = k.strip_prefix('f').and_then(|n| n.parse::<u16>().ok()) {
        return (1..=24).contains(&n).then(|| 0x6F + n);
    }
    let mut chars = k.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphanumeric() => Some(c.to_ascii_uppercase() as u16),
        _ => None,
    }
}

pub fn parse_reload_keys(text: &str) -> ReloadKeys {
    let mut o = ReloadKeys::default();
    for line in text.lines() {
        let Some((key, val)) = line.split_once(':') else { continue };
        match key {
            "key_key.debug.modifier" => o.modifier = mc_key_vk(val).unwrap_or(o.modifier),
            "key_key.debug.reloadResourcePacks" => o.key = mc_key_vk(val).unwrap_or(o.key),
            _ => {}
        }
    }
    o
}

pub fn read_reload_keys(instance: &std::path::Path) -> ReloadKeys {
    std::fs::read_to_string(instance.join("options.txt"))
        .map(|t| parse_reload_keys(&t))
        .unwrap_or_default()
}

/// The game window (HWND as a number), if there is one.
#[cfg_attr(not(test), allow(dead_code))]
pub fn game_window(child_pid: u32) -> Option<usize> {
    let (found, hwnd, _) = platform::find_minecraft_window(child_pid);
    found.then_some(hwnd)
}

/// Live tests: whether the game holds the cursor (playing in the world).
#[cfg_attr(not(test), allow(dead_code))]
pub fn game_cursor_captured(hwnd: usize) -> bool {
    platform::cursor_captured(hwnd)
}

pub fn clamp_interval_secs(secs: u32) -> u32 {
    secs.clamp(5, 3600)
}

pub fn clamp_click_ms(ms: u32) -> u32 {
    ms.clamp(1, 1000)
}

pub fn bind_display(bind: &str) -> String {
    match bind.trim() {
        "" => "Нет".into(),
        "XButton1" => "Мышь 4 (Назад)".into(),
        "XButton2" => "Мышь 5 (Вперёд)".into(),
        "MButton" => "Колёсико".into(),
        "CapsLock" => "CapsLock".into(),
        "Oem3" => "~ (Ё)".into(),
        other => other.to_string(),
    }
}

pub fn parse_bind_vk(bind: &str) -> Option<u16> {
    let key = bind.trim();
    if key.is_empty() || key.eq_ignore_ascii_case("нет") || key.eq_ignore_ascii_case("none") {
        return None;
    }
    match key.to_ascii_uppercase().as_str() {
        "F1" => Some(0x70),
        "F2" => Some(0x71),
        "F3" => Some(0x72),
        "F4" => Some(0x73),
        "F5" => Some(0x74),
        "F6" => Some(0x75),
        "F7" => Some(0x76),
        "F8" => Some(0x77),
        "F9" => Some(0x78),
        "F10" => Some(0x79),
        "F11" => Some(0x7A),
        "F12" => Some(0x7B),
        "MBUTTON" => Some(0x04),
        "XBUTTON1" => Some(0x05),
        "XBUTTON2" => Some(0x06),
        "TAB" => Some(0x09),
        "CAPSLOCK" => Some(0x14),
        "SPACE" => Some(0x20),
        "PAGEUP" => Some(0x21),
        "PAGEDOWN" => Some(0x22),
        "END" => Some(0x23),
        "HOME" => Some(0x24),
        "INSERT" => Some(0x2D),
        "DELETE" => Some(0x2E),
        "OEM3" | "~" | "`" => Some(0xC0),
        s if s.len() == 1 => {
            let b = s.as_bytes()[0];
            if b.is_ascii_uppercase() || b.is_ascii_digit() {
                Some(b as u16)
            } else {
                None
            }
        }
        _ => None,
    }
}

struct RuntimeConfig {
    anti_afk: AtomicBool,
    anti_afk_secs: AtomicU32,
    anti_afk_vk: AtomicU32,
    autoclicker: AtomicBool,
    autoclicker_ms: AtomicU32,
    autoclicker_rmb: AtomicBool,
    autoclicker_vk: AtomicU32,
    capturing_bind: AtomicBool,
    toggle_seq: AtomicU64,
    stop: AtomicBool,
    /// F3+T is owed to the game; keys packed as `modifier << 16 | key`.
    reload_pending: AtomicBool,
    reload_keys: AtomicU32,
}

impl RuntimeConfig {
    fn from_effects(cfg: &EffectsConfig) -> Self {
        Self {
            anti_afk: AtomicBool::new(cfg.anti_afk),
            anti_afk_secs: AtomicU32::new(clamp_interval_secs(cfg.anti_afk_secs)),
            anti_afk_vk: AtomicU32::new(parse_bind_vk(&cfg.anti_afk_bind).unwrap_or(0) as u32),
            autoclicker: AtomicBool::new(cfg.autoclicker),
            autoclicker_ms: AtomicU32::new(clamp_click_ms(cfg.autoclicker_ms)),
            autoclicker_rmb: AtomicBool::new(cfg.autoclicker_rmb),
            autoclicker_vk: AtomicU32::new(parse_bind_vk(&cfg.autoclicker_bind).unwrap_or(0) as u32),
            capturing_bind: AtomicBool::new(false),
            toggle_seq: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            reload_pending: AtomicBool::new(false),
            reload_keys: AtomicU32::new(0),
        }
    }
}

pub struct AutomationHandle {
    rt: Arc<RuntimeConfig>,
    snap: Arc<Mutex<AutomationSnapshot>>,
}

impl Drop for AutomationHandle {
    fn drop(&mut self) {
        self.rt.stop.store(true, Ordering::Relaxed);
    }
}

impl AutomationHandle {
    pub fn start(shared: Arc<Shared>, ctx: egui::Context, initial: &EffectsConfig) -> Self {
        let rt = Arc::new(RuntimeConfig::from_effects(initial));
        let snap = Arc::new(Mutex::new(AutomationSnapshot {
            anti_afk: initial.anti_afk,
            autoclicker: initial.autoclicker,
            ..Default::default()
        }));

        let rt_bg = rt.clone();
        let snap_bg = snap.clone();
        std::thread::spawn(move || worker_loop(shared, ctx, rt_bg, snap_bg));

        Self { rt, snap }
    }

    pub fn update_from_config(&self, cfg: &EffectsConfig) {
        self.rt.anti_afk.store(cfg.anti_afk, Ordering::Relaxed);
        self.rt
            .anti_afk_secs
            .store(clamp_interval_secs(cfg.anti_afk_secs), Ordering::Relaxed);
        self.rt
            .anti_afk_vk
            .store(parse_bind_vk(&cfg.anti_afk_bind).unwrap_or(0) as u32, Ordering::Relaxed);

        self.rt.autoclicker.store(cfg.autoclicker, Ordering::Relaxed);
        self.rt
            .autoclicker_ms
            .store(clamp_click_ms(cfg.autoclicker_ms), Ordering::Relaxed);
        self.rt
            .autoclicker_rmb
            .store(cfg.autoclicker_rmb, Ordering::Relaxed);
        self.rt
            .autoclicker_vk
            .store(parse_bind_vk(&cfg.autoclicker_bind).unwrap_or(0) as u32, Ordering::Relaxed);
    }

    /// The effects pack changed: make the running game reload it (F3+T).
    /// Sent as soon as the game is being played — focused, cursor captured,
    /// no menu open — because Minecraft 26.x (SDL) takes keys only in focus
    /// and ignores debug keys while any screen (pause menu!) is open.
    pub fn request_reload(&self, instance: &std::path::Path) {
        let k = read_reload_keys(instance);
        self.rt
            .reload_keys
            .store(((k.modifier as u32) << 16) | k.key as u32, Ordering::Relaxed);
        self.rt.reload_pending.store(true, Ordering::Relaxed);
    }

    pub fn set_capturing_bind(&self, capturing: bool) {
        self.rt.capturing_bind.store(capturing, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> AutomationSnapshot {
        self.snap.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

fn worker_loop(
    shared: Arc<Shared>,
    ctx: egui::Context,
    rt: Arc<RuntimeConfig>,
    snap: Arc<Mutex<AutomationSnapshot>>,
) {
    let mut prev_ac_down = false;
    let mut prev_afk_down = false;
    let mut last_click = Instant::now();
    let mut last_afk = Instant::now();
    let mut afk_ticks = 0u64;
    let mut click_count = 0u64;
    let mut cached_ext = (false, 0usize, String::new());
    let mut last_scan: Option<Instant> = None;
    let mut prev_anti_afk_on = rt.anti_afk.load(Ordering::Relaxed);
    let mut fine_timer = false;
    let mut cached_fg = FgInfo::default();
    let mut last_fg: Option<Instant> = None;
    let mut reload_seq = 0u64;

    while !rt.stop.load(Ordering::Relaxed) {
        let capturing = rt.capturing_bind.load(Ordering::Relaxed);

        // Global hotkey edges (F6 / F7 / custom binds)
        if !capturing {
            let ac_vk = rt.autoclicker_vk.load(Ordering::Relaxed) as u16;
            let ac_down = ac_vk != 0 && platform::is_vk_down(ac_vk);
            if ac_down && !prev_ac_down {
                let next = !rt.autoclicker.load(Ordering::Relaxed);
                rt.autoclicker.store(next, Ordering::Relaxed);
                rt.toggle_seq.fetch_add(1, Ordering::Relaxed);
                last_click = Instant::now();
                ctx.request_repaint();
            }
            prev_ac_down = ac_down;

            let afk_vk = rt.anti_afk_vk.load(Ordering::Relaxed) as u16;
            let afk_down = afk_vk != 0 && platform::is_vk_down(afk_vk);
            if afk_down && !prev_afk_down {
                let next = !rt.anti_afk.load(Ordering::Relaxed);
                rt.anti_afk.store(next, Ordering::Relaxed);
                rt.toggle_seq.fetch_add(1, Ordering::Relaxed);
                last_afk = Instant::now();
                ctx.request_repaint();
            }
            prev_afk_down = afk_down;
        } else {
            prev_ac_down = false;
            prev_afk_down = false;
        }

        let child_pid = shared.game_pid();
        let child_running = shared.is_game_running();

        // Nothing to automate and no game of ours: only the Effects status
        // strip wants to know about a Minecraft window, so look rarely.
        let idle = !child_running
            && !rt.anti_afk.load(Ordering::Relaxed)
            && !rt.autoclicker.load(Ordering::Relaxed);
        let scan_every = Duration::from_millis(if idle { 2500 } else { 500 });

        // Focus check every tick while it matters; when idle the hotkey
        // poll may still run every 35 ms, the window lookups need not.
        if !idle || last_fg.map(|t| t.elapsed() >= Duration::from_millis(400)).unwrap_or(true) {
            cached_fg = platform::foreground_minecraft(child_pid);
            last_fg = Some(Instant::now());
        }
        let fg = &cached_fg;
        if fg.is_mc {
            cached_ext = (true, fg.hwnd_raw, fg.title.clone());
            last_scan = Some(Instant::now());
        } else if last_scan.map(|t| t.elapsed() >= scan_every).unwrap_or(true) {
            cached_ext = platform::find_minecraft_window(child_pid);
            last_scan = Some(Instant::now());
        }

        let mc_running = child_running || cached_ext.0;
        let mc_focused = mc_running && fg.is_mc;
        let mc_hwnd = if fg.is_mc { fg.hwnd_raw } else { cached_ext.1 };
        let mc_title = if !fg.title.is_empty() {
            fg.title.clone()
        } else {
            cached_ext.2.clone()
        };

        let anti_afk_on = rt.anti_afk.load(Ordering::Relaxed);
        if anti_afk_on && !prev_anti_afk_on {
            last_afk = Instant::now();
        }
        prev_anti_afk_on = anti_afk_on;

        let afk_interval = Duration::from_secs(rt.anti_afk_secs.load(Ordering::Relaxed) as u64);
        let next_afk_in = if anti_afk_on && mc_running {
            let elapsed = last_afk.elapsed();
            if elapsed >= afk_interval {
                // Only send forward micro-tap into the running Minecraft window.
                if platform::send_afk_step(mc_focused, mc_hwnd) {
                    afk_ticks += 1;
                }
                last_afk = Instant::now();
                Some(afk_interval.as_secs() as u32)
            } else {
                Some((afk_interval.saturating_sub(elapsed).as_secs() as u32).max(1))
            }
        } else {
            None
        };

        let autoclicker_on = rt.autoclicker.load(Ordering::Relaxed);
        let click_ms = rt.autoclicker_ms.load(Ordering::Relaxed).clamp(1, 1000) as u64;
        let click_every = Duration::from_millis(click_ms);
        // Only into the focused game window. Captured cursor (in the world):
        // a real click — Windows keeps that cursor inside the game, and
        // Minecraft 26.x reads buttons there only from raw input. Free
        // cursor (inventory, menus): a click posted to the game window, so
        // it never lands on another window or the title bar's ×.
        let clicking = autoclicker_on && mc_running && mc_focused && mc_hwnd != 0 && !capturing;
        // 1 ms timer only while clicking: Windows sleeps in ~15.6 ms steps
        // otherwise, so «10 мс» would really click every ~16 ms.
        if clicking != fine_timer {
            fine_timer = clicking;
            platform::fine_timer(clicking);
        }
        if clicking {
            if last_click.elapsed() >= click_every {
                if platform::click_game(mc_hwnd, rt.autoclicker_rmb.load(Ordering::Relaxed)) {
                    click_count += 1;
                }
                // Fixed cadence (no drift from the time spent sending), but
                // never a burst to «catch up» after a pause.
                last_click += click_every;
                if last_click.elapsed() > click_every * 2 {
                    last_click = Instant::now();
                }
            }
        }

        let mut reload_pending = rt.reload_pending.load(Ordering::Relaxed);
        if reload_pending && !mc_running {
            rt.reload_pending.store(false, Ordering::Relaxed);
            reload_pending = false;
        } else if reload_pending && mc_focused && platform::cursor_captured(mc_hwnd) {
            let keys = rt.reload_keys.load(Ordering::Relaxed);
            if platform::send_reload_keys(mc_hwnd, (keys >> 16) as u16, keys as u16) {
                reload_seq += 1;
            }
            rt.reload_pending.store(false, Ordering::Relaxed);
            reload_pending = false;
        }

        let toggle_seq = rt.toggle_seq.load(Ordering::Relaxed);
        let state_changed = {
            if let Ok(mut guard) = snap.lock() {
                let changed = guard.mc_running != mc_running
                    || guard.mc_focused != mc_focused
                    || guard.anti_afk != anti_afk_on
                    || guard.autoclicker != autoclicker_on
                    || guard.toggle_seq != toggle_seq
                    || guard.reload_pending != reload_pending
                    || guard.reload_seq != reload_seq
                    || guard.mc_title != mc_title;
                guard.mc_running = mc_running;
                guard.mc_focused = mc_focused;
                guard.anti_afk = anti_afk_on;
                guard.autoclicker = autoclicker_on;
                guard.afk_ticks = afk_ticks;
                guard.click_count = click_count;
                guard.reload_pending = reload_pending;
                guard.reload_seq = reload_seq;
                guard.next_afk_in = next_afk_in;
                guard.mc_title = mc_title;
                guard.toggle_seq = toggle_seq;
                changed
            } else {
                false
            }
        };
        if state_changed {
            ctx.request_repaint();
        }

        let sleep_ms = if clicking {
            (click_ms / 2).clamp(1, 15)
        } else if ac_has_bind(&rt) {
            // A hotkey press must still be caught (~40 ms is a short tap).
            35
        } else if idle {
            400
        } else {
            140
        };
        std::thread::sleep(Duration::from_millis(sleep_ms));
    }
    if fine_timer {
        platform::fine_timer(false);
    }
}

fn ac_has_bind(rt: &RuntimeConfig) -> bool {
    rt.autoclicker_vk.load(Ordering::Relaxed) != 0 || rt.anti_afk_vk.load(Ordering::Relaxed) != 0
}

pub fn poll_new_bind() -> Option<BindCapture> {
    platform::poll_new_bind()
}

// ---------------------------------------------------------------- OS implementation

#[derive(Default)]
struct FgInfo {
    is_mc: bool,
    hwnd_raw: usize,
    title: String,
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MouseInput {
        dx: i32,
        dy: i32,
        mouse_data: u32,
        dw_flags: u32,
        time: u32,
        dw_extra_info: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct KeybdInput {
        w_vk: u16,
        w_scan: u16,
        dw_flags: u32,
        time: u32,
        dw_extra_info: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct HardwareInput {
        u_msg: u32,
        w_param_l: u16,
        w_param_h: u16,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    union InputUnion {
        mi: MouseInput,
        ki: KeybdInput,
        hi: HardwareInput,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub(super) struct WinInput {
        input_type: u32,
        u: InputUnion,
    }

    const INPUT_MOUSE: u32 = 0;
    const INPUT_KEYBOARD: u32 = 1;
    const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
    const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
    const MOUSEEVENTF_RIGHTDOWN: u32 = 0x0008;
    const MOUSEEVENTF_RIGHTUP: u32 = 0x0010;
    const WM_LBUTTONDOWN: u32 = 0x0201;
    const WM_LBUTTONUP: u32 = 0x0202;
    const WM_RBUTTONDOWN: u32 = 0x0204;
    const WM_RBUTTONUP: u32 = 0x0205;
    const MK_LBUTTON: usize = 0x0001;
    const MK_RBUTTON: usize = 0x0002;
    const KEYEVENTF_KEYUP: u32 = 0x0002;
    const KEYEVENTF_SCANCODE: u32 = 0x0008;
    const WM_KEYDOWN: u32 = 0x0100;
    const WM_KEYUP: u32 = 0x0101;
    const VK_W: u16 = 0x57;

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> *mut c_void;
        fn GetWindowThreadProcessId(hwnd: *mut c_void, lpdw_process_id: *mut u32) -> u32;
        fn GetClassNameW(hwnd: *mut c_void, lp_class_name: *mut u16, n_max_count: i32) -> i32;
        fn GetWindowTextW(hwnd: *mut c_void, lp_string: *mut u16, n_max_count: i32) -> i32;
        fn IsWindowVisible(hwnd: *mut c_void) -> i32;
        fn EnumWindows(
            lp_enum_func: unsafe extern "system" fn(*mut c_void, isize) -> i32,
            l_param: isize,
        ) -> i32;
        fn GetAsyncKeyState(v_key: i32) -> i16;
        fn MapVirtualKeyW(u_code: u32, u_map_type: u32) -> u32;
        fn SendInput(c_inputs: u32, p_inputs: *const WinInput, cb_size: i32) -> u32;
        fn PostMessageW(hwnd: *mut c_void, msg: u32, w_param: usize, l_param: isize) -> i32;
        fn GetClientRect(hwnd: *mut c_void, rect: *mut WinRect) -> i32;
        fn GetWindowRect(hwnd: *mut c_void, rect: *mut WinRect) -> i32;
        fn GetClipCursor(rect: *mut WinRect) -> i32;
        fn GetCursorInfo(info: *mut CursorInfo) -> i32;
    }

    #[repr(C)]
    #[derive(Default)]
    struct CursorInfo {
        cb_size: u32,
        flags: u32,
        cursor: usize,
        x: i32,
        y: i32,
    }

    /// The game holds the cursor (playing in the world): hidden and clipped
    /// inside the game window. Free cursor = some screen is open.
    pub(super) fn cursor_captured(hwnd: usize) -> bool {
        const CURSOR_SHOWING: u32 = 1;
        if hwnd == 0 {
            return false;
        }
        let mut info = CursorInfo { cb_size: std::mem::size_of::<CursorInfo>() as u32, ..Default::default() };
        let (mut clip, mut win) = (WinRect::default(), WinRect::default());
        // SAFETY: plain out-parameters of the right size.
        let ok = unsafe {
            GetCursorInfo(&mut info) != 0
                && GetClipCursor(&mut clip) != 0
                && GetWindowRect(hwnd as *mut c_void, &mut win) != 0
        };
        ok && info.flags & CURSOR_SHOWING == 0
            && clip.left >= win.left
            && clip.top >= win.top
            && clip.right <= win.right
            && clip.bottom <= win.bottom
    }

    /// One autoclicker click into the focused game (see the worker).
    pub(super) fn click_game(hwnd: usize, rmb: bool) -> bool {
        if cursor_captured(hwnd) {
            send_input_click(rmb)
        } else {
            send_mouse_click(hwnd, rmb)
        }
    }

    /// A real click at the cursor — only while the game clips it.
    fn send_input_click(rmb: bool) -> bool {
        let (down, up) = if rmb {
            (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP)
        } else {
            (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP)
        };
        let ev = |flags| WinInput {
            input_type: INPUT_MOUSE,
            u: InputUnion {
                mi: MouseInput { dx: 0, dy: 0, mouse_data: 0, dw_flags: flags, time: 0, dw_extra_info: 0 },
            },
        };
        let inputs = [ev(down), ev(up)];
        // SAFETY: a valid array of INPUT structs of the declared size.
        unsafe { SendInput(2, inputs.as_ptr(), std::mem::size_of::<WinInput>() as i32) == 2 }
    }

    #[repr(C)]
    #[derive(Default)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn is_foreground(hwnd: usize) -> bool {
        hwnd != 0 && unsafe { GetForegroundWindow() } as usize == hwnd
    }

    #[link(name = "winmm")]
    extern "system" {
        fn timeBeginPeriod(u_period: u32) -> u32;
        fn timeEndPeriod(u_period: u32) -> u32;
    }

    /// F3+T into the game window as posted key messages, sent while the
    /// game is focused and played (SDL in 26.x drops keys of an unfocused
    /// window). Scan codes ride in lParam — GLFW and SDL map keys by them.
    pub(super) fn send_reload_keys(hwnd: usize, modifier: u16, key: u16) -> bool {
        if hwnd == 0 {
            return false;
        }
        let h = hwnd as *mut c_void;
        let lp = |vk: u16, up: bool| {
            let sc = unsafe { MapVirtualKeyW(vk as u32, 0) } as isize;
            let flags = if up { (1isize << 30) | (1isize << 31) } else { 0 };
            1isize | (sc << 16) | flags
        };
        // SAFETY: posting messages to a window handle only queues them; a
        // stale handle makes PostMessageW fail, nothing else.
        [(WM_KEYDOWN, modifier), (WM_KEYDOWN, key), (WM_KEYUP, key), (WM_KEYUP, modifier)]
            .into_iter()
            .map(|(msg, vk)| unsafe { PostMessageW(h, msg, vk as usize, lp(vk, msg == WM_KEYUP)) } != 0)
            .fold(true, |ok, sent| ok && sent)
    }

    pub(super) fn fine_timer(on: bool) {
        unsafe {
            if on {
                timeBeginPeriod(1);
            } else {
                timeEndPeriod(1);
            }
        }
    }

    fn win_text(hwnd: *mut c_void, class: bool) -> String {
        let mut buf = [0u16; 160];
        let len = unsafe {
            if class {
                GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32)
            } else {
                GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32)
            }
        };
        if len <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..len as usize])
    }

    fn matches_minecraft_window(hwnd: *mut c_void, child_pid: u32) -> Option<String> {
        if hwnd.is_null() {
            return None;
        }
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return None;
        }
        let title = win_text(hwnd, false);
        if title == crate::config::APP_TITLE {
            return None;
        }
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if child_pid != 0 && pid == child_pid {
            return Some(if title.is_empty() { "Minecraft".into() } else { title });
        }
        if title.is_empty() {
            return None;
        }
        // A game started elsewhere: a GLFW window is not enough (other games
        // use GLFW too, and the autoclicker must never click there).
        let class_name = win_text(hwnd, true);
        let mc_title = title.to_ascii_lowercase().contains("minecraft");
        if ((class_name == "GLFW30" || class_name == "SDL_app") && mc_title) || class_name == "LWJGL" {
            return Some(title);
        }
        None
    }

    pub(super) fn is_vk_down(vk: u16) -> bool {
        (unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000) != 0
    }

    pub(super) fn foreground_minecraft(child_pid: u32) -> FgInfo {
        let hwnd = unsafe { GetForegroundWindow() };
        if let Some(title) = matches_minecraft_window(hwnd, child_pid) {
            FgInfo {
                is_mc: true,
                hwnd_raw: hwnd as usize,
                title,
            }
        } else {
            FgInfo::default()
        }
    }

    struct EnumState {
        child_pid: u32,
        found_hwnd: usize,
        title: String,
    }

    unsafe extern "system" fn enum_cb(hwnd: *mut c_void, lparam: isize) -> i32 {
        let st = &mut *(lparam as *mut EnumState);
        if let Some(title) = matches_minecraft_window(hwnd, st.child_pid) {
            st.found_hwnd = hwnd as usize;
            st.title = title;
            return 0;
        }
        1
    }

    pub(super) fn find_minecraft_window(child_pid: u32) -> (bool, usize, String) {
        let mut st = EnumState {
            child_pid,
            found_hwnd: 0,
            title: String::new(),
        };
        unsafe { EnumWindows(enum_cb, &mut st as *mut EnumState as isize) };
        (st.found_hwnd != 0, st.found_hwnd, st.title)
    }

    /// A click posted to the game window itself (client centre), not
    /// through the real cursor: it cannot hit another window or the ×.
    pub(super) fn send_mouse_click(hwnd: usize, rmb: bool) -> bool {
        if hwnd == 0 {
            return false;
        }
        let h = hwnd as *mut c_void;
        let mut r = WinRect::default();
        unsafe { GetClientRect(h, &mut r) };
        let (x, y) = (((r.right - r.left) / 2) as isize, ((r.bottom - r.top) / 2) as isize);
        let pos = (x & 0xFFFF) | ((y & 0xFFFF) << 16);
        let (down, up, held) = if rmb {
            (WM_RBUTTONDOWN, WM_RBUTTONUP, MK_RBUTTON)
        } else {
            (WM_LBUTTONDOWN, WM_LBUTTONUP, MK_LBUTTON)
        };
        // SAFETY: only queues messages; a stale handle fails the call.
        unsafe { PostMessageW(h, down, held, pos) != 0 && PostMessageW(h, up, 0, pos) != 0 }
    }

    pub(super) fn send_afk_step(mc_focused: bool, mc_hwnd: usize) -> bool {
        let scan = unsafe { MapVirtualKeyW(VK_W as u32, 0) } as u16;
        if mc_focused {
            let down = WinInput {
                input_type: INPUT_KEYBOARD,
                u: InputUnion {
                    ki: KeybdInput {
                        w_vk: VK_W,
                        w_scan: scan,
                        dw_flags: KEYEVENTF_SCANCODE,
                        time: 0,
                        dw_extra_info: 0,
                    },
                },
            };
            let up = WinInput {
                input_type: INPUT_KEYBOARD,
                u: InputUnion {
                    ki: KeybdInput {
                        w_vk: VK_W,
                        w_scan: scan,
                        dw_flags: KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dw_extra_info: 0,
                    },
                },
            };
            let sz = std::mem::size_of::<WinInput>() as i32;
            unsafe {
                SendInput(1, &down, sz);
            }
            std::thread::sleep(Duration::from_millis(115));
            unsafe {
                SendInput(1, &up, sz);
            }
            return true;
        }
        if mc_hwnd != 0 {
            let hwnd = mc_hwnd as *mut c_void;
            let l_down = 1isize | ((scan as isize) << 16);
            let l_up = 1isize | ((scan as isize) << 16) | (1isize << 30) | (1isize << 31);
            unsafe {
                PostMessageW(hwnd, WM_KEYDOWN, VK_W as usize, l_down);
            }
            std::thread::sleep(Duration::from_millis(115));
            unsafe {
                PostMessageW(hwnd, WM_KEYUP, VK_W as usize, l_up);
            }
            return true;
        }
        false
    }

    pub(super) fn poll_new_bind() -> Option<BindCapture> {
        if is_vk_down(0x1B) {
            return Some(BindCapture::Cancel);
        }
        if is_vk_down(0x08) {
            return Some(BindCapture::Clear);
        }
        const CANDIDATES: &[(u16, &str)] = &[
            (0x04, "MButton"),
            (0x05, "XButton1"),
            (0x06, "XButton2"),
            (0x70, "F1"),
            (0x71, "F2"),
            (0x72, "F3"),
            (0x73, "F4"),
            (0x74, "F5"),
            (0x75, "F6"),
            (0x76, "F7"),
            (0x77, "F8"),
            (0x78, "F9"),
            (0x79, "F10"),
            (0x7A, "F11"),
            (0x7B, "F12"),
            (0x14, "CapsLock"),
            (0x2D, "Insert"),
            (0x2E, "Delete"),
            (0x24, "Home"),
            (0x23, "End"),
            (0x21, "PageUp"),
            (0x22, "PageDown"),
            (0xC0, "Oem3"),
        ];
        for &(vk, name) in CANDIDATES {
            if is_vk_down(vk) {
                return Some(BindCapture::Set(name.to_string()));
            }
        }
        for b in b'A'..=b'Z' {
            if is_vk_down(b as u16) {
                return Some(BindCapture::Set((b as char).to_string()));
            }
        }
        for b in b'0'..=b'9' {
            if is_vk_down(b as u16) {
                return Some(BindCapture::Set((b as char).to_string()));
            }
        }
        None
    }

    #[cfg(test)]
    pub(super) fn input_struct_size() -> usize {
        std::mem::size_of::<WinInput>()
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;

    pub(super) fn is_vk_down(_vk: u16) -> bool {
        false
    }

    pub(super) fn foreground_minecraft(_child_pid: u32) -> FgInfo {
        FgInfo::default()
    }

    pub(super) fn find_minecraft_window(_child_pid: u32) -> (bool, usize, String) {
        (false, 0, String::new())
    }

    pub(super) fn click_game(_hwnd: usize, _rmb: bool) -> bool {
        false
    }

    pub(super) fn cursor_captured(_hwnd: usize) -> bool {
        false
    }

    pub(super) fn send_reload_keys(_hwnd: usize, _modifier: u16, _key: u16) -> bool {
        false
    }

    pub(super) fn fine_timer(_on: bool) {}

    pub(super) fn send_afk_step(_mc_focused: bool, _mc_hwnd: usize) -> bool {
        false
    }

    pub(super) fn poll_new_bind() -> Option<BindCapture> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_binds_and_clamps_values() {
        assert_eq!(parse_bind_vk("F6"), Some(0x75));
        assert_eq!(parse_bind_vk("xbutton2"), Some(0x06));
        assert_eq!(parse_bind_vk("r"), Some(b'R' as u16));
        assert_eq!(parse_bind_vk("5"), Some(b'5' as u16));
        assert_eq!(parse_bind_vk("Нет"), None);
        assert_eq!(parse_bind_vk(""), None);

        assert_eq!(clamp_interval_secs(1), 5);
        assert_eq!(clamp_interval_secs(60), 60);
        assert_eq!(clamp_interval_secs(9000), 3600);

        assert_eq!(clamp_click_ms(0), 1);
        assert_eq!(clamp_click_ms(10), 10);
        assert_eq!(clamp_click_ms(5000), 1000);
    }

    #[cfg(all(windows, target_pointer_width = "64"))]
    /// Live helper: posts input to a running Minecraft window, to see what
    /// the game really takes. `MADRID_POST=lmb|rmb|f3` (10 clicks / F3).
    /// `cargo test --bin sad-launcher live_post_input -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_post_input() {
        let what = std::env::var("MADRID_POST").unwrap_or_else(|_| "lmb".into());
        let (found, hwnd, title) = platform::find_minecraft_window(0);
        assert!(found, "no Minecraft window");
        println!(
            "окно «{title}», в фокусе={}, курсор захвачен={}",
            platform::is_foreground(hwnd),
            platform::cursor_captured(hwnd)
        );
        for _ in 0..10 {
            match what.as_str() {
                // Fullbright on / off in the effects pack, then F3+T.
                "bright" | "plain" => {
                    let cfg = crate::config::load();
                    let mc = std::env::var("MADRID_LIVE_MC").unwrap_or_else(|_| "26.3".into());
                    let mut fx = crate::effects::EffectsConfig::default();
                    fx.fullbright = what == "bright";
                    let jar = crate::effects::client_jar(&cfg, &mc);
                    let rep = crate::effects::apply_to_instance(&cfg.instance_dir_for("default"), &mc, &fx, jar.as_deref()).unwrap();
                    let keys = read_reload_keys(&cfg.instance_dir_for("default"));
                    println!("пак записан={}, F3+T={}", rep.pack_written, platform::send_reload_keys(hwnd, keys.modifier, keys.key));
                    break;
                }
                "f3" => {
                    platform::send_reload_keys(hwnd, 0x72, 0x72);
                    break;
                }
                _ => {
                    // The launcher's own choice: real click when captured.
                    platform::click_game(hwnd, what == "rmb");
                }
            }
            std::thread::sleep(Duration::from_millis(150));
        }
    }

    #[test]
    fn reads_reload_keys_from_options() {
        assert_eq!(parse_reload_keys(""), ReloadKeys::default());
        let o = parse_reload_keys(
            "pauseOnLostFocus:false\nkey_key.debug.modifier:key.keyboard.f4\nkey_key.debug.reloadResourcePacks:key.keyboard.r\n",
        );
        assert_eq!((o.modifier, o.key), (0x73, 0x52));
        // Unsupported binding (a mouse button) → keep the default key.
        let o = parse_reload_keys("key_key.debug.reloadResourcePacks:key.mouse.left\n");
        assert_eq!(o.key, 0x54);
        assert_eq!(mc_key_vk("key.keyboard.7"), Some(0x37));
        assert_eq!(mc_key_vk("key.keyboard.f12"), Some(0x7B));
        assert_eq!(mc_key_vk("key.keyboard.left.shift"), None);
    }

    #[test]
    fn win64_input_struct_matches_winuser_abi() {
        assert_eq!(platform::input_struct_size(), 40);
    }
}
