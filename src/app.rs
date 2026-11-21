//! Launcher shell: window chrome, navigation, background jobs, persistence.

use crate::config::{self, Config, VERSION};
use crate::icons::{self, Icon};
use crate::java::{self, JavaEvent, JavaInfo};
use crate::launch;
use crate::loader::Spec;
use crate::minecraft::{self, InstallEvent, InstalledVersion};
use crate::mods::{self, BuildEntry, ModEvent, ModHit};
use crate::net;
use crate::pomme::{self, PommeEvent};
use crate::server;
use crate::shared::{LogKind, Shared};
use crate::theme::{self, Skin};
use crate::widgets;
use egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, Id, Layout, Pos2, Rect, RichText, Sense,
    Stroke, Ui, Vec2,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

static PUBLIC_IP: std::sync::OnceLock<String> = std::sync::OnceLock::new();

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Play,
    Versions,
    Build,
    Mods,
    Effects,
    Server,
    Settings,
}

#[derive(Clone)]
pub struct NavEntry {
    pub tab: Tab,
    pub mods_page: usize,
    pub detail: Option<ModHit>,
}

impl NavEntry {
    fn same(&self, other: &Self) -> bool {
        self.tab == other.tab
            && self.mods_page == other.mods_page
            && self.detail.as_ref().map(|d| (&d.source, &d.id))
                == other.detail.as_ref().map(|d| (&d.source, &d.id))
    }
}

impl Tab {
    pub fn all() -> [Tab; 7] {
        [
            Tab::Play,
            Tab::Versions,
            Tab::Build,
            Tab::Mods,
            Tab::Effects,
            Tab::Server,
            Tab::Settings,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Play => "Старт",
            Tab::Versions => "Версии",
            Tab::Build => "Сборка",
            Tab::Mods => "Моды",
            Tab::Effects => "Эффекты",
            Tab::Server => "Игра с другом",
            Tab::Settings => "Настройки",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Tab::Play => Icon::Compass,
            Tab::Versions => Icon::Cube,
            Tab::Build => Icon::Layers,
            Tab::Mods => Icon::Course,
            Tab::Effects => Icon::Effects,
            Tab::Server => Icon::Server,
            Tab::Settings => Icon::Sliders,
        }
    }
}

pub struct Toast {
    pub text: String,
    pub at: Instant,
}

#[derive(Clone, Debug)]
pub enum FriendState {
    Working(String),
    Ok(String),
    Err(String),
}

pub struct UpdateTask {
    pub stage: String,
    pub done: u64,
    pub total: u64,
    pub finished: bool,
    pub ready_path: Option<PathBuf>,
    pub error: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub rx: Receiver<crate::updater::UpdateEvent>,
}

impl UpdateTask {
    pub fn frac(&self) -> f32 {
        if self.total > 0 {
            (self.done as f32 / self.total as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn pump(&mut self) -> bool {
        loop {
            match self.rx.try_recv() {
                Ok(crate::updater::UpdateEvent::Stage(s)) => self.stage = s,
                Ok(crate::updater::UpdateEvent::Progress { done, total }) => {
                    self.done = done;
                    self.total = total;
                }
                Ok(crate::updater::UpdateEvent::Ready(p)) => {
                    self.ready_path = Some(p);
                    self.finished = true;
                    return false;
                }
                Ok(crate::updater::UpdateEvent::Error(e)) => {
                    self.error = Some(e);
                    self.finished = true;
                    return false;
                }
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => {
                    if !self.finished {
                        self.finished = true;
                        if self.error.is_none() {
                            self.error = Some("Загрузка обновления прервана".into());
                        }
                    }
                    return false;
                }
            }
        }
    }
}

struct RemoveCard {
    last: crate::remove::Progress,
    shown_at: Instant,
    done_at: Option<Instant>,
    freed: u64,
}

pub struct InstallTask {
    /// What is being installed.
    pub spec: Spec,
    /// Version id it produced (known when done).
    pub version: String,
    pub stage: String,
    pub done: u64,
    pub total: u64,
    pub finished: bool,
    pub ok: bool,
    pub error: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub auto_launch: bool,
    pub rx: Receiver<InstallEvent>,
}

impl InstallTask {
    pub fn frac(&self) -> f32 {
        if self.total > 0 {
            (self.done as f32 / self.total as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Returns true while the install is still running.
    pub fn pump(&mut self) -> bool {
        loop {
            match self.rx.try_recv() {
                Ok(InstallEvent::Stage(s)) => self.stage = s,
                Ok(InstallEvent::Progress { done, total, label }) => {
                    self.done = done;
                    self.total = total;
                    self.stage = label;
                }
                Ok(InstallEvent::Done { version }) => {
                    self.version = version;
                    self.ok = true;
                    self.finished = true;
                    return false;
                }
                Ok(InstallEvent::Error(e)) => {
                    self.error = Some(e);
                    self.finished = true;
                    return false;
                }
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => {
                    if !self.finished {
                        self.finished = true;
                        if self.error.is_none() {
                            self.error = Some("Загрузка прервана".into());
                        }
                    }
                    return false;
                }
            }
        }
    }
}

/// Text without the characters the loaded fonts cannot draw (colour
/// emoji, private-use icons, joiners) — egui shows those as empty boxes.
/// Lines that started with such a «bullet» lose the leftover space too.
pub fn drawable_text(ctx: &egui::Context, text: &str) -> String {
    let font = crate::theme::body(13.0);
    let mut out = String::with_capacity(text.len());
    ctx.fonts_mut(|f| {
        for line in text.split('\n') {
            let kept: String = line
                .chars()
                .filter(|&c| {
                    c == ' '
                        || c == '\t'
                        || (!matches!(c, '\u{200d}' | '\u{fe0f}' | '\u{fe0e}') && f.has_glyph(&font, c))
                })
                .collect();
            out.push_str(kept.trim_start());
            out.push('\n');
        }
    });
    out.pop();
    out
}

/// A description translation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Translation {
    Working,
    /// Text, and whether glyph-less characters were already stripped.
    Done(String, bool),
    Failed(String),
}

/// The video in the viewer: starting, playing, or why it cannot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VideoState {
    Loading,
    Playing,
    Error(String),
}

/// Whether an installed pack build is the newest version of the pack.
#[derive(Clone)]
pub enum PackCheck {
    Checking,
    Current,
    Update(crate::mods::PackFile),
    Failed,
}

pub struct ModTask {
    pub title: String,
    pub kind: String,
    pub done: u64,
    pub total: u64,
    pub finished: bool,
    pub error: Option<String>,
    pub warn: Option<String>,
    pub path: Option<PathBuf>,
    pub cancel: Arc<AtomicBool>,
    pub last_file: String,
    pub pack_meta: Option<(String, Spec)>,
    /// Bytes of the current phase (pack files): the bar follows them, the
    /// «5/120» counter follows files. (0, 0) — no byte sizes known.
    pub bytes: (u64, u64),
    pub rx: Receiver<ModEvent>,
}

impl ModTask {
    pub fn frac(&self) -> f32 {
        if self.bytes.1 > 0 {
            (self.bytes.0 as f32 / self.bytes.1 as f32).clamp(0.0, 1.0)
        } else if self.total > 0 {
            (self.done as f32 / self.total as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn pump(&mut self) -> bool {
        loop {
            match self.rx.try_recv() {
                Ok(ModEvent::Progress { done, total }) => {
                    if done == 0 {
                        // A new phase starts: its own counter, no stale bytes.
                        self.bytes = (0, 0);
                    }
                    self.done = done;
                    self.total = total;
                }
                Ok(ModEvent::Bytes { done, total }) => self.bytes = (done, total),
                Ok(ModEvent::File(name)) => self.last_file = name,
                Ok(ModEvent::PackMeta { profile_id, spec }) => {
                    self.pack_meta = Some((profile_id, spec));
                }
                Ok(ModEvent::Warn(w)) => self.warn = Some(w),
                Ok(ModEvent::Done(path)) => {
                    self.path = Some(path);
                    self.finished = true;
                    return false;
                }
                Ok(ModEvent::Error(e)) => {
                    self.error = Some(e);
                    self.finished = true;
                    return false;
                }
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => {
                    if !self.finished {
                        self.finished = true;
                        if self.path.is_none() && self.error.is_none() {
                            self.error = Some("Загрузка прервана".into());
                        }
                    }
                    return false;
                }
            }
        }
    }
}

/// In-app Temurin JRE download: the launcher installs Java itself.
pub struct JavaTask {
    pub major: u32,
    pub done: u64,
    pub total: u64,
    pub finished: bool,
    pub error: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub rx: Receiver<JavaEvent>,
}

impl JavaTask {
    pub fn frac(&self) -> f32 {
        if self.total > 0 {
            (self.done as f32 / self.total as f32).clamp(0.0, 1.0)
        } else {
            0.05
        }
    }

    pub fn pump(&mut self) -> bool {
        loop {
            match self.rx.try_recv() {
                Ok(JavaEvent::Progress { done, total }) => {
                    self.done = done;
                    self.total = total;
                }
                Ok(JavaEvent::Done(_path)) => {
                    self.finished = true;
                    self.error = None;
                    return false;
                }
                Ok(JavaEvent::Error(e)) => {
                    self.error = Some(e);
                    self.finished = true;
                    return false;
                }
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => {
                    if !self.finished {
                        self.finished = true;
                        if self.error.is_none() {
                            self.error = Some("Загрузка прервана".into());
                        }
                    }
                    return false;
                }
            }
        }
    }
}

/// Pomme download: one portable exe from GitHub releases.
pub struct PommeTask {
    pub stage: String,
    pub done: u64,
    pub total: u64,
    pub finished: bool,
    pub error: Option<String>,
    pub rx: Receiver<PommeEvent>,
}

impl PommeTask {
    pub fn frac(&self) -> f32 {
        if self.total > 0 {
            (self.done as f32 / self.total as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn pump(&mut self) -> bool {
        loop {
            match self.rx.try_recv() {
                Ok(PommeEvent::Stage(s)) => self.stage = s,
                Ok(PommeEvent::Progress { done, total }) => {
                    self.done = done;
                    self.total = total;
                }
                Ok(PommeEvent::Done) => {
                    self.finished = true;
                    self.error = None;
                    return false;
                }
                Ok(PommeEvent::Error(e)) => {
                    self.error = Some(e);
                    self.finished = true;
                    return false;
                }
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => {
                    if !self.finished {
                        self.finished = true;
                        if self.error.is_none() {
                            self.error = Some("Загрузка прервана".into());
                        }
                    }
                    return false;
                }
            }
        }
    }
}

pub enum BuildEvent {
    File(String),
    Ok(PathBuf),
    Err(String),
}

/// Installs every mod of the user's build, one by one.
pub struct BuildTask {
    pub total: usize,
    pub done: usize,
    pub finished: bool,
    pub error: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub last_file: String,
    pub rx: Receiver<BuildEvent>,
}

impl BuildTask {
    pub fn frac(&self) -> f32 {
        if self.total > 0 {
            (self.done as f32 / self.total as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn pump(&mut self) -> bool {
        loop {
            match self.rx.try_recv() {
                Ok(BuildEvent::File(name)) => {
                    self.last_file = name;
                }
                Ok(BuildEvent::Ok(_path)) => {
                    self.done += 1;
                    if self.done >= self.total {
                        self.finished = true;
                        return false;
                    }
                }
                Ok(BuildEvent::Err(e)) => {
                    self.error = Some(e);
                    self.finished = true;
                    return false;
                }
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => {
                    if !self.finished {
                        self.finished = true;
                    }
                    return false;
                }
            }
        }
    }
}

pub enum EffectsModMsg {
    Stage(String),
    Finished(Result<(Vec<String>, Vec<String>), String>),
}

pub struct EffectsModTask {
    pub stage: String,
    pub finished: bool,
    pub installed: Vec<String>,
    pub missing: Vec<String>,
    pub error: Option<String>,
    pub cancel: Arc<AtomicBool>,
    rx: Receiver<EffectsModMsg>,
}

impl EffectsModTask {
    pub fn pump(&mut self) -> bool {
        let mut active = false;
        loop {
            match self.rx.try_recv() {
                Ok(EffectsModMsg::Stage(s)) => {
                    self.stage = s;
                    active = true;
                }
                Ok(EffectsModMsg::Finished(Ok((ins, mis)))) => {
                    self.installed = ins;
                    self.missing = mis;
                    self.finished = true;
                    return false;
                }
                Ok(EffectsModMsg::Finished(Err(e))) => {
                    self.error = Some(e);
                    self.finished = true;
                    return false;
                }
                Err(TryRecvError::Empty) => return active,
                Err(TryRecvError::Disconnected) => {
                    self.finished = true;
                    return false;
                }
            }
        }
    }
}

/// What the last update check said (shown next to «Проверить обновления»).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    Latest,
    Found(String),
    Offline,
}

/// One finished catalog request. `gen` ties it to the request that is
/// current in the UI — answers to older requests only feed the cache.
pub struct SearchResult {
    pub gen: u64,
    pub offset: usize,
    pub key: String,
    pub source: String,
    pub prefetch: bool,
    pub result: Result<Vec<ModHit>, String>,
}

pub struct SearchState {
    pub query: String,
    /// 0 = CurseForge, 1 = Modrinth.
    pub source: usize,
    /// Index into mods::KINDS — mods, shaders, resourcepacks, maps, packs.
    pub kind: usize,
    /// 0 = downloads, 1 = follows (популярность), 2 = updated, 3 = newest.
    pub sort: usize,
    /// Pagination: offset of the next page to load.
    pub offset: usize,
    /// No more pages on the current query.
    pub exhausted: bool,
    pub busy: bool,
    pub results: Vec<ModHit>,
    pub error: Option<String>,
    pub searched: bool,
    /// Bumped on every new request; stale answers are ignored.
    pub gen: u64,
    /// Fetched pages by request key: flipping chips back is instant.
    pub cache: HashMap<String, (Instant, Vec<ModHit>)>,
    /// When the first page of the current results appeared (card fade-in).
    pub shown_at: Instant,
    /// Last keystroke in the search field — search fires after a pause.
    pub typed_at: Option<Instant>,
    /// Build (MC + loader) the shown results were fetched for.
    pub spec_key: String,
    /// Selected category id ("" = all): Modrinth slug or CurseForge id.
    pub category: String,
    /// Filter typed inside the category dropdown.
    pub category_filter: String,
}

impl SearchState {
    fn new() -> Self {
        Self {
            query: String::new(),
            source: 1,
            kind: 0,
            sort: 0,
            offset: 0,
            exhausted: false,
            busy: false,
            results: Vec::new(),
            error: None,
            searched: false,
            gen: 0,
            cache: HashMap::new(),
            shown_at: Instant::now(),
            typed_at: None,
            spec_key: String::new(),
            category: String::new(),
            category_filter: String::new(),
        }
    }

    /// Clears results for a new query/filter; the next search starts at 0.
    pub fn reset(&mut self) {
        self.offset = 0;
        self.exhausted = false;
        self.searched = false;
        self.results.clear();
        self.error = None;
    }
}

/// Search pages are reused this long — download counts and «обновлено»
/// stay close to live (a card refreshes its own tile at once).
const CACHE_TTL: Duration = Duration::from_secs(180);

pub struct Launcher {
    pub cfg: Config,
    pub shared: Arc<Shared>,
    pub tab: Tab,
    /// When the current tab was opened — drives the page transition.
    pub tab_at: Instant,
    pub install: Option<InstallTask>,
    pub mod_task: Option<ModTask>,
    pub java_task: Option<JavaTask>,
    pub build_task: Option<BuildTask>,
    pub pomme_task: Option<PommeTask>,
    pub search: SearchState,
    search_tx: Sender<SearchResult>,
    search_rx: Receiver<SearchResult>,
    pub toasts: Vec<Toast>,
    /// Background removal card: last seen progress, when it appeared and
    /// when it finished (the card lingers a moment, then scales away).
    remove_card: Option<RemoveCard>,
    remove_seen: u64,
    pub last_alpha: f32,
    pub confirm_delete: Option<String>,
    pub confirm_uninstall: bool,
    pub confirm_reset: bool,
    /// «Удалить все сборки» asks once before wiping.
    pub confirm_wipe: bool,
    /// «О лаунчере» panel: when it was opened (None = closed).
    about_at: Option<Instant>,
    pub versions_filter: String,
    pub versions_all: bool,
    pub was_running: bool,
    pub was_mc_running: bool,
    /// Server the game should join once an install finishes.
    pub pending_target: Option<String>,
    /// Joined online room, if any.
    pub room: Option<crate::room::Room>,
    pub room_code_buf: String,
    pub host_port_buf: String,
    /// Router port forward: (port, in-flight answer).
    pub upnp_rx: Option<(u16, Receiver<Result<crate::upnp::Opened, String>>)>,
    /// Last router answer for the hosted port.
    pub upnp_state: Option<(u16, Result<crate::upnp::Opened, String>)>,
    pub wallpaper_key: String,
    pub wallpaper_tex: Option<egui::TextureHandle>,
    pub skin_key: String,
    pub skin_tex: Option<egui::TextureHandle>,
    pub icon_tex: HashMap<String, egui::TextureHandle>,
    pub icon_fetching: HashSet<String>,
    pub icon_failed: HashMap<String, Instant>,
    icon_tx: Sender<(String, Option<egui::ColorImage>)>,
    icon_rx: Receiver<(String, Option<egui::ColorImage>)>,
    pub mods_cache: Vec<crate::mods::InstalledMod>,
    pub mods_at: Instant,
    mods_stamp: Option<(PathBuf, Option<std::time::SystemTime>)>,
    mods_full_at: Instant,
    update_checked_at: Instant,
    /// Last measured data-folder size; filled by a background walk.
    pub storage_bytes: Arc<std::sync::atomic::AtomicU64>,
    storage_busy: Arc<AtomicBool>,
    pub storage_at: Instant,
    pub lan_ip: Option<String>,
    pub build: Vec<BuildEntry>,
    /// 0 = каталог, 1 = установленные
    pub mods_page: usize,
    pub catalog_at: Instant,
    pub detail: Option<crate::mods::ModHit>,
    /// When the project card opened — drives its slide-in.
    pub detail_at: Instant,
    pub detail_body: Option<Result<crate::mods::ProjectDetail, String>>,
    pub detail_rx: Option<Receiver<Result<crate::mods::ProjectDetail, String>>>,
    /// Newest version of each installed pack build, asked live from its
    /// source (profile id → state); filled in the background.
    pub pack_checks: Arc<std::sync::Mutex<HashMap<String, PackCheck>>>,
    /// «Перевести на русский»: translations of descriptions by project key
    /// (`source:id`), filled in the background.
    pub translations: Arc<std::sync::Mutex<HashMap<String, Translation>>>,
    /// Projects whose card shows the translation instead of the original.
    pub show_translated: HashSet<String>,
    /// Telegram-style media viewer over the whole window.
    pub viewer: Option<crate::views::viewer::Viewer>,
    /// Player state of the video shown in the viewer.
    pub video_state: Option<VideoState>,
    /// This frame's request: play this id over this rect (points).
    pub video_request: Option<(String, Rect)>,
    /// The YouTube player window (WebView2), alive while a card with videos
    /// or the viewer is open.
    #[cfg(windows)]
    pub video: Option<crate::video::VideoView>,
    /// WebView2 could not start: videos open in the browser instead.
    pub video_unavailable: Option<String>,
    pub public_ip_cached: Option<String>,
    pub public_ip_fetching: bool,
    pub friend_code: String,
    pub friend_port: String,
    pub friend_state: Option<FriendState>,
    pub update_info: Option<crate::updater::UpdateInfo>,
    update_rx: Option<Receiver<Result<Option<crate::updater::UpdateInfo>, ()>>>,
    /// Result of the last check, shown by «Проверить обновления».
    pub update_status: UpdateStatus,
    /// The running check was started by the button (not the silent timer).
    update_manual: bool,
    pub update_task: Option<UpdateTask>,
    pub update_dismissed: bool,
    update_shown_at: Option<Instant>,
    pub nav_history: Vec<NavEntry>,
    pub nav_pos: usize,
    nav_lock: bool,
    pub middle_scroll_anchor: Option<Pos2>,
    middle_scroll_dragged: bool,
    pub focus_search: bool,
    pub effects: crate::effects::EffectsConfig,
    pub effects_status: Option<(bool, String)>,
    /// Effects changed but not yet written to effects.json (slider drag).
    pub effects_dirty: bool,
    /// Set packs being downloaded (Modrinth) for the effects pack.
    pub rp_fetch_rx: Option<Receiver<Result<Vec<String>, String>>>,
    /// Settings changed but config.json not yet written (slider drag).
    pub cfg_dirty: bool,
    /// options.txt / SadEffects.zip check for the Effects tab: re-read at
    /// most once a second or when the settings change, not every frame.
    effects_applied: Option<(PathBuf, crate::effects::EffectsConfig, Instant, crate::effects::AppliedState)>,
    pub effects_mod_task: Option<EffectsModTask>,
    pub automation: crate::automation::AutomationHandle,
    pub auto_snap: crate::automation::AutomationSnapshot,
    auto_seen_toggle_seq: u64,
    auto_seen_reload_seq: u64,
    pub bind_capture: Option<crate::automation::BindTarget>,
    // first-run nickname screen
    pub nick_buf: String,
    pub ctx: Option<egui::Context>,
    // custom build / profiles tab
    pub build_selected_id: String,
    pub build_new_name: String,
    pub build_new_mc: String,
    pub build_new_loader: String,
    pub build_creating: bool,
    pub build_search: String,
    pub build_confirm_del: Option<String>,
    /// Live download speed (global byte counter, smoothed).
    pub speed: net::SpeedMeter,
    /// Modpack size/count by `source:id`.
    pub pack_info: HashMap<String, crate::packinfo::PackInfo>,
    pub pack_info_busy: HashSet<String>,
    pack_info_dirty: bool,
    pack_info_tx: Sender<(String, Option<crate::packinfo::PackInfo>)>,
    pack_info_rx: Receiver<(String, Option<crate::packinfo::PackInfo>)>,
    /// The launcher minimised itself for the game — restore when it ends.
    pub minimized_for_game: bool,
    /// «Играть» waits for a mod download (FTB fill) and then runs.
    pub launch_after_mods: Option<Option<String>>,
    /// Builds already checked for FTB mods this session (one try each).
    pub ftb_tried: HashSet<String>,
    /// Missing-dependency sets already fetched this session (no loops).
    pub deps_tried: HashSet<String>,
    started: Instant,
    trimmed: bool,
    trimmed_start: bool,
    was_focused: bool,
    last_trim: Instant,
}

/// Disk cache path for a remote image.
fn image_cache_path(url: &str) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    config::data_dir()
        .join("meta")
        .join("icons")
        .join(format!("{:016x}.img", h.finish()))
}

/// Decodes and downsizes an image off the UI thread.
fn decode_image(bytes: &[u8], max_side: u32) -> Option<egui::ColorImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let img = if img.width() > max_side || img.height() > max_side {
        img.thumbnail(max_side, max_side)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        rgba.as_raw(),
    ))
}

/// Fetches (disk cache first) and decodes a remote image.
async fn load_image(url: String, max_side: u32) -> Option<egui::ColorImage> {
    let file = image_cache_path(&url);
    if let Ok(b) = tokio::fs::read(&file).await {
        if !b.is_empty() {
            return tokio::task::spawn_blocking(move || decode_image(&b, max_side)).await.ok().flatten();
        }
    }
    // Only the network part waits for a slot: icons already on disk never
    // queue behind slow downloads.
    let _permit = image_permits().acquire_owned().await;
    let resp = net::client()
        .get(&url)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    // Bytes → Vec without a copy when the buffer is not shared.
    let bytes: Vec<u8> = resp.bytes().await.ok()?.into();
    tokio::task::spawn_blocking(move || {
        let img = decode_image(&bytes, max_side)?;
        // The disk keeps what is shown, not the original: a 1–2 MB banner
        // becomes a ~40 KB thumbnail. Small originals are kept as they are.
        let to_store = if bytes.len() > 48 * 1024 {
            encode_png(&img).filter(|png| png.len() < bytes.len()).unwrap_or(bytes)
        } else {
            bytes
        };
        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&file, &to_store);
        Some(img)
    })
    .await
    .ok()
    .flatten()
}

/// Re-encodes a decoded image as PNG for the disk cache.
fn encode_png(img: &egui::ColorImage) -> Option<Vec<u8>> {
    let [w, h] = img.size;
    let mut raw = Vec::with_capacity(w * h * 4);
    for p in &img.pixels {
        let [r, g, b, a] = p.to_srgba_unmultiplied();
        raw.extend_from_slice(&[r, g, b, a]);
    }
    let buf = image::RgbaImage::from_raw(w as u32, h as u32, raw)?;
    let mut out = std::io::Cursor::new(Vec::new());
    buf.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

fn image_permits() -> Arc<tokio::sync::Semaphore> {
    static SEM: std::sync::OnceLock<Arc<tokio::sync::Semaphore>> = std::sync::OnceLock::new();
    // Icons come from two CDNs (Modrinth over HTTP/2, forgecdn): 16 in
    // flight hides their latency; decoding runs on the blocking pool.
    SEM.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(16))).clone()
}

impl Launcher {
    /// Refreshes cheap caches at most once a second.
    fn refresh_caches(&mut self) {
        if self.mods_at.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.mods_at = Instant::now();
        // A folder's mtime changes when a jar is added, removed or renamed
        // (enable/disable), so an unchanged stamp means an unchanged list —
        // one stat instead of re-listing hundreds of jars every second.
        // Every 30 s the list is re-read anyway (a jar overwritten in place).
        let dir = self.cfg.mods_dir();
        let stamp = std::fs::metadata(&dir).and_then(|m| m.modified()).ok();
        let fresh = self.mods_full_at.elapsed() < Duration::from_secs(30)
            && self
                .mods_stamp
                .as_ref()
                .map(|(d, t)| *d == dir && *t == stamp)
                .unwrap_or(false);
        if fresh {
            return;
        }
        self.mods_cache = mods::installed(&self.cfg);
        self.mods_stamp = Some((dir, stamp));
        self.mods_full_at = Instant::now();
    }

    /// Forces the installed-mods list to re-read on the next frame.
    pub fn touch_mods(&mut self) {
        self.mods_at = Instant::now() - Duration::from_secs(3);
        self.mods_stamp = None;
    }

    /// Heavy storage walk for the settings tab, at most once per 4 seconds.
    /// The walk over game/, assets and every build is thousands of files:
    /// it runs on a blocking worker, the UI shows the last number meanwhile.
    pub fn storage_size(&mut self) -> u64 {
        if self.storage_at.elapsed() >= Duration::from_secs(4) && !self.storage_busy.swap(true, Ordering::SeqCst) {
            self.storage_at = Instant::now();
            let out = self.storage_bytes.clone();
            let busy = self.storage_busy.clone();
            let ctx = self.ctx.clone();
            net::rt().spawn_blocking(move || {
                let size = crate::util::dir_size(&config::data_dir());
                let changed = out.swap(size, Ordering::Relaxed) != size;
                busy.store(false, Ordering::SeqCst);
                if let (true, Some(ctx)) = (changed, ctx) {
                    ctx.request_repaint();
                }
            });
        }
        self.storage_bytes.load(Ordering::Relaxed)
    }

    /// Real skin face as a texture, loaded from the chosen skin png.
    pub fn skin_face(&mut self, ctx: &egui::Context) -> Option<&egui::TextureHandle> {
        let path = self.cfg.skin_file.trim().to_string();
        if path.is_empty() {
            if !self.skin_key.is_empty() {
                self.skin_key.clear();
                self.skin_tex = None;
            }
            return None;
        }
        // The key only changes when the path does; the file is re-read when
        // the user picks a skin again (settings resets skin_key).
        if path == self.skin_key {
            return self.skin_tex.as_ref();
        }
        self.skin_key = path.clone();
        let tex = std::fs::read(&path).ok().and_then(|bytes| {
            let img = image::load_from_memory(&bytes).ok()?;
            let rgba = img.to_rgba8();
            // The face lives at (8..16, 8..16) of the standard 64x64 layout.
            let (w, h) = rgba.dimensions();
            let (x0, y0) = if w >= 64 && h >= 64 { (8u32, 8u32) } else { (0, 0) };
            let mut pixels = Vec::with_capacity(64);
            for y in y0..(y0 + 8).min(h) {
                for x in x0..(x0 + 8).min(w) {
                    let p = rgba.get_pixel(x, y);
                    pixels.push(egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]));
                }
            }
            if pixels.len() != 64 {
                return None;
            }
            Some(ctx.load_texture(
                "skin-face",
                egui::ColorImage::new([8, 8], pixels),
                egui::TextureOptions::NEAREST,
            ))
        });
        self.skin_tex = tex;
        self.skin_tex.as_ref()
    }

    /// Catalog icon (≤256 px).
    pub fn mod_icon(&mut self, ctx: &egui::Context, url: &str) -> Option<egui::TextureHandle> {
        // Tiles are 144 px wide: 192 px covers 125 % scaling and costs
        // ~45 % of the memory a 256 px texture does.
        self.remote_image(ctx, url, 192)
    }

    /// Remote image as a texture: memory → disk cache → network. Fetching
    /// and decoding run on the shared runtime; this call never blocks.
    pub fn remote_image(&mut self, ctx: &egui::Context, url: &str, max_side: u32) -> Option<egui::TextureHandle> {
        if url.is_empty() {
            return None;
        }
        if let Some(tex) = self.icon_tex.get(url) {
            return Some(tex.clone());
        }
        if self.icon_fetching.contains(url) {
            return None;
        }
        if let Some(failed_at) = self.icon_failed.get(url) {
            if failed_at.elapsed() < Duration::from_secs(60) {
                return None;
            }
        }
        self.icon_fetching.insert(url.to_string());
        let tx = self.icon_tx.clone();
        let ctx = ctx.clone();
        let url = url.to_string();
        net::spawn(async move {
            let img = load_image(url.clone(), max_side).await;
            let _ = tx.send((url, img));
            ctx.request_repaint();
        });
        None
    }

    /// Uploads freshly decoded images (a few per frame keeps it smooth).
    fn drain_images(&mut self, ctx: &egui::Context) {
        for _ in 0..12 {
            let Ok((url, img)) = self.icon_rx.try_recv() else {
                break;
            };
            self.icon_fetching.remove(&url);
            match img {
                Some(img) => {
                    let tex = ctx.load_texture(format!("img:{url}"), img, egui::TextureOptions::LINEAR);
                    self.icon_tex.insert(url, tex);
                }
                None => {
                    self.icon_failed.insert(url, Instant::now());
                }
            }
        }
        // Keep GPU memory bounded on long catalog sessions.
        if self.icon_tex.len() > 240 {
            let keep: HashSet<String> = self
                .search
                .results
                .iter()
                .filter_map(|h| h.icon_url.clone())
                .collect();
            self.icon_tex.retain(|k, _| keep.contains(k));
        }
    }

    /// «212 модов · 1,4 ГБ» for a pack card: cached, else fetched in the
    /// background (four at a time) and shown as soon as it arrives.
    /// `deep` (open project card) may read a whole CurseForge archive to
    /// turn «архив 49 МБ» into a real mod count.
    pub fn pack_info(&mut self, hit: &ModHit, deep: bool) -> Option<crate::packinfo::PackInfo> {
        let key = format!("{}:{}", hit.source, hit.id);
        let cached = self.pack_info.get(&key).copied();
        let want_deep = deep && cached.map(|i| i.mods == 0).unwrap_or(false);
        if cached.is_some() && !want_deep {
            return cached;
        }
        let busy_key = if want_deep { format!("deep:{key}") } else { key.clone() };
        if self.pack_info_busy.contains(&busy_key) {
            return cached;
        }
        self.pack_info_busy.insert(busy_key.clone());
        let tx = self.pack_info_tx.clone();
        let cfg = self.cfg.clone();
        let hit = hit.clone();
        let ctx = self.ctx.clone();
        net::spawn(async move {
            static PERMITS: std::sync::OnceLock<Arc<tokio::sync::Semaphore>> = std::sync::OnceLock::new();
            let sem = PERMITS.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(4))).clone();
            let _permit = sem.acquire_owned().await;
            let info = crate::packinfo::fetch(&net::client(), &cfg, &hit, want_deep || deep).await.ok();
            let _ = tx.send((busy_key, info));
            if let Some(ctx) = ctx {
                ctx.request_repaint_after(Duration::from_millis(60));
            }
        });
        cached
    }

    fn drain_pack_info(&mut self) {
        while let Ok((busy_key, info)) = self.pack_info_rx.try_recv() {
            // Failed lookups stay «busy»: no retry storm on every frame.
            if let Some(info) = info {
                self.pack_info_busy.remove(&busy_key);
                let key = busy_key.trim_start_matches("deep:").to_string();
                let better = self.pack_info.get(&key).map(|old| old.mods == 0 || info.mods > 0).unwrap_or(true);
                if better {
                    self.pack_info.insert(key, info);
                    self.pack_info_dirty = true;
                }
            }
        }
        if self.pack_info_dirty && self.pack_info_busy.is_empty() {
            self.pack_info_dirty = false;
            if self.pack_info.len() > 400 {
                self.pack_info.clear();
            }
            crate::packinfo::save_cache(&self.pack_info);
        }
    }

    /// Live speed while anything downloads, e.g. «12,4 МБ/с».
    pub fn speed_label(&self) -> String {
        self.speed.label()
    }

    pub fn build_add(&mut self, hit: &ModHit, spec: &Spec) {
        if hit.kind == mods::KIND_MODPACK {
            self.toast("Модпак ставится кнопкой «Скачать»");
            return;
        }
        if self.build.iter().any(|e| e.slug == hit.slug && e.source == hit.source) {
            self.toast("Уже в сборке");
            return;
        }
        self.build.push(BuildEntry {
            source: hit.source.clone(),
            id: hit.id.clone(),
            slug: hit.slug.clone(),
            title: hit.title.clone(),
            kind: hit.kind.clone(),
            mc: spec.mc.clone(),
            loader: spec.loader.id().to_string(),
        });
        mods::save_build(&self.build);
        self.toast(format!("В сборке: {}", hit.title));
    }

    pub fn build_remove(&mut self, idx: usize) {
        if idx < self.build.len() {
            self.build.remove(idx);
            mods::save_build(&self.build);
        }
    }

    pub fn start_build_install(&mut self, ctx: &egui::Context) {
        if self.build_task.as_ref().map(|t| !t.finished).unwrap_or(false) {
            self.toast("Сборка уже ставится");
            return;
        }
        if self.build.is_empty() {
            self.toast("Сборка пуста — добавь моды плюсиком");
            return;
        }
        let spec = self.active_spec();
        let entries = self.build.clone();
        let cfg = self.cfg.clone();
        let (tx, rx) = std::sync::mpsc::channel::<BuildEvent>();
        let ctx = ctx.clone();
        let total = entries.len();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = cancel.clone();
        net::spawn(async move {
            let client = net::client();
            for entry in &entries {
                if cancel_task.load(Ordering::Relaxed) {
                    break;
                }
                let _ = tx.send(BuildEvent::File(entry.title.clone()));
                let hit = ModHit {
                    source: entry.source.clone(),
                    id: entry.id.clone(),
                    slug: entry.slug.clone(),
                    title: entry.title.clone(),
                    summary: String::new(),
                    downloads: 0,
                    author: String::new(),
                    kind: entry.kind.clone(),
                    icon_url: None,
                    file_id: None,
                    file_url: None,
                    file_name: None,
                    tags: String::new(),
                    categories: Vec::new(),
                    updated: String::new(),
                };
                // The build installs into the ACTIVE build's Minecraft/loader.
                let mut noop = |_ev: ModEvent| {};
                let res = mods::install(&client, &cfg, &hit, &spec, &entry.kind, &cancel_task, &mut noop)
                    .await
                    .map_err(|e| format!("{}: {e:#}", entry.title));
                let ev = match res {
                    Ok(p) => BuildEvent::Ok(p),
                    Err(e) => BuildEvent::Err(e),
                };
                if tx.send(ev).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        self.build_task = Some(BuildTask {
            total,
            done: 0,
            finished: false,
            error: None,
            cancel,
            last_file: String::new(),
            rx,
        });
    }
}

impl Launcher {
    pub fn new(cc: &eframe::CreationContext<'_>, mut cfg: Config, shared: Arc<Shared>) -> Self {
        theme::install(&cc.egui_ctx);
        theme::init_motion();
        let skin = Skin::new(cfg.transparency);
        theme::style(&cc.egui_ctx, &skin);

        cfg.autostart = crate::util::autostart_enabled();
        // Java probing spawns processes — keep it off the first frame.
        {
            let shared = shared.clone();
            let custom = cfg.java_path.clone();
            std::thread::spawn(move || shared.set_java(java::probe(&custom)));
        }
        shared.set_installed(minecraft::installed_versions(&cfg));
        if shared.versions().is_empty() {
            minecraft::spawn_manifest(shared.clone());
        }

        let ctx = cc.egui_ctx.clone();
        let (search_tx, search_rx) = std::sync::mpsc::channel();
        let (icon_tx, icon_rx) = std::sync::mpsc::channel();
        let (pack_info_tx, pack_info_rx) = std::sync::mpsc::channel();
        let init_host_port = cfg.host_port.to_string();
        let init_friend_port = cfg.server_port.to_string();
        let initial_effects = crate::effects::load();
        let automation = crate::automation::AutomationHandle::start(
            shared.clone(),
            ctx.clone(),
            &initial_effects,
        );
        let auto_snap = automation.snapshot();
        let mut app = Self {
            ctx: Some(ctx.clone()),
            cfg,
            shared,
            tab: Tab::Play,
            tab_at: Instant::now(),
            install: None,
            mod_task: None,
            java_task: None,
            build_task: None,
            pomme_task: None,
            search: SearchState::new(),
            search_tx,
            search_rx,
            toasts: Vec::new(),
            remove_card: None,
            remove_seen: crate::remove::finished_count(),
            last_alpha: 0.0,
            confirm_delete: None,
            confirm_uninstall: false,
            confirm_reset: false,
            confirm_wipe: false,
            about_at: None,
            versions_filter: String::new(),
            versions_all: false,
            was_running: false,
            was_mc_running: false,
            pending_target: None,
            room: None,
            room_code_buf: String::new(),
            host_port_buf: init_host_port,
            upnp_rx: None,
            upnp_state: None,
            wallpaper_key: String::new(),
            wallpaper_tex: None,
            skin_key: String::new(),
            skin_tex: None,
            icon_tex: HashMap::new(),
            icon_fetching: HashSet::new(),
            icon_failed: HashMap::new(),
            icon_tx,
            icon_rx,
            mods_cache: Vec::new(),
            mods_at: Instant::now() - Duration::from_secs(3),
            mods_stamp: None,
            mods_full_at: Instant::now(),
            update_checked_at: Instant::now(),
            storage_bytes: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            storage_busy: Arc::new(AtomicBool::new(false)),
            storage_at: Instant::now() - Duration::from_secs(5),
            lan_ip: server::local_ip(),
            build: mods::load_build(),
            mods_page: 0,
            catalog_at: Instant::now(),
            detail: None,
            detail_at: Instant::now(),
            detail_body: None,
            detail_rx: None,
            pack_checks: Arc::new(std::sync::Mutex::new(HashMap::new())),
            translations: Arc::new(std::sync::Mutex::new(HashMap::new())),
            show_translated: HashSet::new(),
            viewer: None,
            video_state: None,
            video_request: None,
            #[cfg(windows)]
            video: None,
            video_unavailable: None,
            public_ip_cached: None,
            public_ip_fetching: false,
            friend_code: String::new(),
            friend_port: init_friend_port,
            friend_state: None,
            update_info: None,
            update_rx: None,
            update_status: UpdateStatus::Idle,
            update_manual: false,
            update_task: None,
            update_dismissed: false,
            update_shown_at: None,
            nav_history: Vec::new(),
            nav_pos: 0,
            nav_lock: false,
            middle_scroll_anchor: None,
            middle_scroll_dragged: false,
            focus_search: false,
            effects: initial_effects,
            effects_status: None,
            effects_applied: None,
            effects_dirty: false,
            rp_fetch_rx: None,
            cfg_dirty: false,
            effects_mod_task: None,
            automation,
            auto_snap,
            auto_seen_toggle_seq: 0,
            auto_seen_reload_seq: 0,
            bind_capture: None,
            nick_buf: String::new(),
            build_selected_id: String::new(),
            build_new_name: String::new(),
            build_new_mc: crate::config::MADRID_MC.to_string(),
            build_new_loader: "forge".to_string(),
            build_creating: false,
            build_search: String::new(),
            build_confirm_del: None,
            speed: net::SpeedMeter::default(),
            pack_info: crate::packinfo::load_cache(),
            pack_info_busy: HashSet::new(),
            pack_info_dirty: false,
            pack_info_tx,
            pack_info_rx,
            minimized_for_game: false,
            launch_after_mods: None,
            ftb_tried: HashSet::new(),
            deps_tried: HashSet::new(),
            started: Instant::now(),
            trimmed: false,
            trimmed_start: false,
            was_focused: true,
            last_trim: Instant::now(),
        };
        // Developer screenshots: MADRID_DEV_ZOOM=2 renders everything at 2×
        // (main.rs grows the window to match).
        if let Some(z) = std::env::var("MADRID_DEV_ZOOM").ok().and_then(|z| z.parse::<f32>().ok()) {
            ctx.set_zoom_factor(z.clamp(0.5, 3.0));
        }
        // Developer preview: MADRID_DEV_ABOUT=1 opens «О лаунчере».
        if std::env::var_os("MADRID_DEV_ABOUT").is_some() {
            app.about_at = Some(Instant::now());
        }
        // The catalog is warm before the user ever opens the Mods tab.
        // Developer preview: MADRID_DEV_TAB=modpacks opens the pack catalog.
        if let Ok(tab) = std::env::var("MADRID_DEV_TAB") {
            app.tab = match tab.as_str() {
                "play" => Tab::Play,
                "versions" => Tab::Versions,
                "build" => Tab::Build,
                "effects" => Tab::Effects,
                "server" => Tab::Server,
                "settings" => Tab::Settings,
                _ => Tab::Mods,
            };
            if tab == "modpacks" {
                app.search.kind = mods::KINDS.iter().position(|(k, _)| *k == mods::KIND_MODPACK).unwrap_or(0);
            }
            if let Ok(q) = std::env::var("MADRID_DEV_QUERY") {
                app.search.query = q;
            }
        }
        app.start_search(&ctx);
        app.nav_history.push(NavEntry {
            tab: app.tab,
            mods_page: app.mods_page,
            detail: None,
        });
        crate::updater::cleanup_temp_installer();
        app.check_update(&ctx);
        app
    }

    /// Asks GitHub for a newer version: at start-up, then every 6 hours for
    /// a launcher left open (the user hears about a release without restart).
    fn check_update(&mut self, ctx: &egui::Context) {
        let dev_copy = std::env::var_os("MADRID_SECOND_INSTANCE").is_some();
        if dev_copy && std::env::var_os("MADRID_DEV_UPDATE").is_none() {
            return;
        }
        self.start_update_check(ctx, false);
    }

    /// «Проверить обновления»: asks right now and reports the answer, even
    /// a version the user chose «Не показывать» for.
    pub fn check_update_now(&mut self, ctx: &egui::Context) {
        if self.update_rx.is_some() {
            return;
        }
        self.start_update_check(ctx, true);
    }

    fn start_update_check(&mut self, ctx: &egui::Context, manual: bool) {
        self.update_checked_at = Instant::now();
        self.update_manual = manual;
        if manual {
            self.update_status = UpdateStatus::Checking;
        }
        let (utx, urx) = std::sync::mpsc::channel();
        self.update_rx = Some(urx);
        let uctx = ctx.clone();
        net::spawn(async move {
            let found = crate::updater::probe(&net::client(), config::VERSION).await;
            let _ = utx.send(found);
            uctx.request_repaint();
        });
    }

    /// Version, last check result and the check / update button — the same
    /// row in «О лаунчере» and at the bottom of «Настройки».
    pub fn update_row(&mut self, ui: &mut Ui, skin: &Skin) {
        let ctx = ui.ctx().clone();
        let (text, color) = match &self.update_status {
            UpdateStatus::Idle => (format!("Версия {VERSION}"), skin.dim()),
            UpdateStatus::Checking => (format!("Версия {VERSION} · проверяю…"), skin.dim()),
            UpdateStatus::Latest => (format!("Версия {VERSION} · установлена последняя"), skin.green()),
            UpdateStatus::Found(v) => (format!("Доступна версия {v}"), skin.green()),
            UpdateStatus::Offline => ("Нет связи с GitHub · попробуйте позже".to_string(), skin.red()),
        };
        let found = matches!(self.update_status, UpdateStatus::Found(_));
        let checking = matches!(self.update_status, UpdateStatus::Checking);
        let mut clicked = false;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(text).font(theme::body(12.5)).color(color));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = if found { "Обновиться" } else { "Проверить обновления" };
                let btn = if found { widgets::Btn::Primary } else { widgets::Btn::Outline };
                clicked = widgets::button_icon(ui, skin, Icon::Refresh, label, btn, !checking, 28.0, 11.0, 11.5).clicked();
            });
        });
        if clicked {
            if found {
                if let Some(info) = self.update_info.clone() {
                    self.update_dismissed = false;
                    self.start_update_download(&ctx, info);
                }
            } else {
                self.check_update_now(&ctx);
            }
        }
    }

    pub fn save_cfg(&self) {
        let mut cfg = self.cfg.clone();
        let nick = crate::util::sanitize_nickname(&cfg.nickname);
        if crate::util::nickname_valid(&nick) {
            cfg.nickname = nick;
        }
        // Invalid (too short) nicknames keep the previous one — never saved.
        config::save(&cfg);
        self.shared.set_cfg(cfg);
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        let text = text.into();
        // The same message twice in a row just refreshes its timer.
        if let Some(last) = self.toasts.last_mut() {
            if last.text == text {
                last.at = Instant::now();
                return;
            }
        }
        self.toasts.push(Toast {
            text,
            at: Instant::now(),
        });
        if self.toasts.len() > 3 {
            self.toasts.remove(0);
        }
    }

    pub fn go(&mut self, tab: Tab) {
        if self.tab != tab {
            self.tab = tab;
            self.tab_at = Instant::now();
        }
    }

    pub fn refresh_installed(&self) {
        self.shared
            .set_installed(minecraft::installed_versions(&self.cfg));
    }

    pub fn reprobe_java(&self) {
        java::invalidate_probe();
        self.shared.set_java(java::probe(&self.cfg.java_path));
    }

    pub fn installed(&self) -> Vec<InstalledVersion> {
        self.shared.installed()
    }

    /// Minecraft + loader of the ACTIVE build. A modpack build targets its
    /// own release and loader — «Играть» runs exactly that.
    pub fn active_spec(&self) -> Spec {
        crate::profiles::get_active_profile(&self.cfg).spec()
    }

    /// The installed version id that runs the active build, if any.
    pub fn compatible_version(&self) -> Option<String> {
        let installed = minecraft::as_installed(&self.installed());
        crate::loader::pick_installed(&installed, &self.active_spec())
    }

    pub fn start_java_download(&mut self, ctx: &egui::Context, major: u32) {
        if self.java_task.as_ref().map(|t| !t.finished).unwrap_or(false) {
            self.toast("Java уже скачивается");
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<JavaEvent>();
        let ctx = ctx.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = cancel.clone();
        net::spawn(async move {
            let client = net::client();
            let mut send = |ev: JavaEvent| {
                let _ = tx.send(ev);
                ctx.request_repaint_after(Duration::from_millis(90));
            };
            if let Err(e) = java::download_runtime(&client, major, &cancel_task, &mut send).await {
                send(JavaEvent::Error(format!("{e:#}")));
            }
        });
        self.java_task = Some(JavaTask {
            major,
            done: 0,
            total: 0,
            finished: false,
            error: None,
            cancel,
            rx,
        });
    }

    pub fn start_server(&mut self) {
        if self.shared.is_mc_running() || self.shared.is_mc_busy() {
            self.toast("Сервер уже занят");
            return;
        }
        server::start_async(self.shared.clone());
        self.toast("Поднимаю сервер Мадрид");
    }

    pub fn stop_server(&mut self) {
        if server::stop(&self.shared) {
            self.toast("Сервер останавливается…");
        }
    }

    /// Loads (once) the user's wallpaper image behind the window slab.
    pub fn ensure_wallpaper(&mut self, ctx: &egui::Context) {
        let path = self.cfg.wallpaper.trim().to_string();
        if path.is_empty() {
            if !self.wallpaper_key.is_empty() {
                self.wallpaper_key.clear();
                self.wallpaper_tex = None;
            }
            return;
        }
        // Only a new path triggers a reload — no metadata syscall per frame.
        if path == self.wallpaper_key {
            return;
        }
        self.wallpaper_key = path.clone();
        let Ok(bytes) = std::fs::read(&path) else {
            return;
        };
        if let Some(img) = decode_image(&bytes, 1920) {
            self.wallpaper_tex = Some(ctx.load_texture("wallpaper", img, egui::TextureOptions::LINEAR));
        }
    }

    // ------------------------------------------------------------------ jobs

    /// Installs a build's Minecraft + loader (+ Java when missing).
    pub fn start_install(&mut self, ctx: &egui::Context, spec: Spec, auto_launch: bool) {
        if let Some(t) = self.install.as_mut() {
            if !t.finished {
                if t.spec == spec {
                    t.auto_launch |= auto_launch;
                    self.toast("Уже ставлю — игра откроется сама");
                } else {
                    self.toast("Установка уже идёт");
                }
                return;
            }
        }
        let (tx, rx) = std::sync::mpsc::channel::<InstallEvent>();
        let cfg = self.cfg.clone();
        let ctx = ctx.clone();
        let spec_task = spec.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = cancel.clone();
        net::spawn(async move {
            let client = net::client();
            let mut send = |ev: InstallEvent| {
                let _ = tx.send(ev);
                ctx.request_repaint_after(Duration::from_millis(90));
            };
            if let Err(e) = minecraft::install(&client, &cfg, &spec_task, &cancel_task, &mut send).await {
                send(InstallEvent::Error(format!("{e:#}")));
            }
        });
        self.install = Some(InstallTask {
            spec,
            version: String::new(),
            stage: "Подготовка…".into(),
            done: 0,
            total: 0,
            finished: false,
            ok: false,
            error: None,
            cancel,
            auto_launch,
            rx,
        });
    }

    fn search_request(&self) -> (String, String, String, String, Spec, String) {
        let kind = mods::KINDS[self.search.kind.min(mods::KINDS.len() - 1)].0.to_string();
        let wanted = if self.search.source == 0 { "curseforge" } else { "modrinth" };
        let source = mods::effective_source(&kind, wanted).to_string();
        let sort = match self.search.sort {
            1 => "follows",
            2 => "updated",
            3 => "newest",
            _ => "downloads",
        }
        .to_string();
        (
            self.search.query.trim().to_string(),
            source,
            kind,
            sort,
            self.active_spec(),
            self.search.category.clone(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn search_key(query: &str, source: &str, kind: &str, sort: &str, spec: &Spec, category: &str, offset: usize) -> String {
        format!(
            "{source}|{kind}|{sort}|{}|{}|{category}|{query}|{offset}",
            spec.mc,
            spec.loader.id()
        )
    }

    /// Loads the page at `search.offset` for the current filters. A newer
    /// request always wins: nothing waits for an older one to finish.
    pub fn start_search(&mut self, ctx: &egui::Context) {
        let (query, source, kind, sort, spec, category) = self.search_request();
        let offset = self.search.offset;
        self.search.gen += 1;
        let gen = self.search.gen;
        let key = Self::search_key(&query, &source, &kind, &sort, &spec, &category, offset);
        if let Some((at, hits)) = self.search.cache.get(&key) {
            if at.elapsed() < CACHE_TTL {
                let hits = hits.clone();
                self.apply_search(offset, &source, Ok(hits));
                self.prefetch_next(ctx);
                return;
            }
        }
        self.search.busy = true;
        self.search.error = None;
        self.spawn_search(ctx, gen, offset, key, false, query, source, kind, sort, spec, category);
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_search(
        &self,
        ctx: &egui::Context,
        gen: u64,
        offset: usize,
        key: String,
        prefetch: bool,
        query: String,
        source: String,
        kind: String,
        sort: String,
        spec: Spec,
        category: String,
    ) {
        let tx = self.search_tx.clone();
        let cfg = self.cfg.clone();
        let ctx = ctx.clone();
        net::spawn(async move {
            let result = mods::search(&net::client(), &cfg, &query, &source, &spec, &kind, &sort, &category, offset)
                .await
                .map_err(|e| format!("{e:#}"));
            let _ = tx.send(SearchResult {
                gen,
                offset,
                key,
                source,
                prefetch,
                result,
            });
            ctx.request_repaint();
        });
    }

    /// Quietly fetches the next page into the cache: «Показать ещё» is instant.
    fn prefetch_next(&mut self, ctx: &egui::Context) {
        if self.search.exhausted || self.search.results.is_empty() {
            return;
        }
        let (query, source, kind, sort, spec, category) = self.search_request();
        let offset = self.search.offset;
        let key = Self::search_key(&query, &source, &kind, &sort, &spec, &category, offset);
        if self.search.cache.contains_key(&key) {
            return;
        }
        self.spawn_search(ctx, self.search.gen, offset, key, true, query, source, kind, sort, spec, category);
    }

    fn apply_search(&mut self, offset: usize, source: &str, result: Result<Vec<ModHit>, String>) {
        self.search.busy = false;
        self.search.searched = true;
        match result {
            Ok(list) => {
                let fresh = list.len();
                self.search.exhausted = fresh < mods::page_size(source);
                if offset > 0 {
                    let known: HashSet<(String, String)> = self
                        .search
                        .results
                        .iter()
                        .map(|h| (h.source.clone(), h.id.clone()))
                        .collect();
                    self.search
                        .results
                        .extend(list.into_iter().filter(|h| !known.contains(&(h.source.clone(), h.id.clone()))));
                } else {
                    self.search.results = list;
                    self.search.shown_at = Instant::now();
                }
                self.search.offset = offset + fresh;
                self.search.error = None;
                self.catalog_at = Instant::now();
            }
            Err(e) => {
                if offset == 0 {
                    self.search.results.clear();
                }
                self.search.error = Some(e);
            }
        }
    }

    fn drain_search(&mut self, ctx: &egui::Context) {
        while let Ok(r) = self.search_rx.try_recv() {
            if let Ok(hits) = &r.result {
                self.search.cache.insert(r.key.clone(), (Instant::now(), hits.clone()));
            }
            if r.prefetch || r.gen != self.search.gen {
                continue;
            }
            let ok = r.result.is_ok();
            self.apply_search(r.offset, &r.source, r.result);
            if ok {
                self.prefetch_next(ctx);
                // With the catalog on screen, icons of the next rows start
                // loading now, not when each card scrolls into view. Not on
                // other tabs: the start-up search must not cost RAM/CPU.
                let on_catalog = self.tab == Tab::Mods;
                let urls: Vec<String> = self
                    .search
                    .results
                    .iter()
                    .skip(r.offset)
                    .take(if on_catalog { 25 } else { 0 })
                    .filter_map(|h| h.icon_url.clone())
                    .collect();
                for url in urls {
                    let _ = self.mod_icon(ctx, &url);
                }
            }
        }
        if self.search.cache.len() > 120 {
            self.search.cache.retain(|_, (at, _)| at.elapsed() < CACHE_TTL);
        }
    }

    /// Keeps the YouTube player window in step with the UI: created hidden
    /// as soon as a card with videos opens (the first video already loads,
    /// so a click plays at once), placed over the viewer's media rect while
    /// a video is shown, hidden otherwise, dropped when neither the card nor
    /// the viewer needs it (it holds ~100 МБ).
    fn sync_video(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let request = self.video_request.take();
        #[cfg(windows)]
        {
            let card_videos: Vec<String> = match (&self.detail, &self.detail_body) {
                (Some(_), Some(Ok(d))) => d.videos.clone(),
                _ => Vec::new(),
            };
            let wanted = request.as_ref().map(|(id, _)| id.clone()).or_else(|| card_videos.first().cloned());
            let Some(first) = wanted else {
                self.video = None;
                return;
            };
            if self.video.is_none() && self.video_unavailable.is_none() {
                match crate::video::VideoView::new(frame, &first) {
                    Ok(v) => self.video = Some(v),
                    Err(e) => {
                        self.shared.log(crate::shared::LogKind::Warn, format!("Видео в лаунчере: {e:#}"));
                        self.video_unavailable = Some(format!("{e:#}"));
                    }
                }
            }
            if let Some(err) = &self.video_unavailable {
                if request.is_some() && self.video_state.is_none() {
                    self.video_state = Some(VideoState::Error(format!("Встроенный плеер недоступен ({err}) — открой на YouTube")));
                }
                return;
            }
            let Some(video) = self.video.as_mut() else {
                return;
            };
            for ev in video.take_events() {
                use crate::video::PlayerEvent;
                match ev {
                    PlayerEvent::Playing => {
                        if self.video_state != Some(VideoState::Playing) {
                            self.shared.log(crate::shared::LogKind::Info, format!("Видео {} играет в лаунчере", video.current));
                        }
                        self.video_state = Some(VideoState::Playing);
                    }
                    PlayerEvent::Error(code) => {
                        self.shared.log(crate::shared::LogKind::Warn, format!("Видео {}: ошибка плеера {code}", video.current));
                        if request.is_some() {
                            self.video_state = Some(VideoState::Error(crate::video::error_text(code).to_string()));
                        }
                    }
                    _ => {}
                }
            }
            match request {
                Some((id, rect)) => {
                    let ppp = ctx.pixels_per_point();
                    video.place((
                        (rect.left() * ppp).round() as i32,
                        (rect.top() * ppp).round() as i32,
                        (rect.width() * ppp).round() as u32,
                        (rect.height() * ppp).round() as u32,
                    ));
                    if self.video_state.is_none() {
                        self.video_state = Some(VideoState::Loading);
                        video.play(&id);
                    }
                    video.show(true);
                }
                None => video.show(false),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (ctx, frame);
            // No embedded player here: the viewer opens the video page.
            if let Some((id, _)) = request {
                if self.video_state.is_none() {
                    self.video_state = Some(VideoState::Error("Видео откроется на YouTube".into()));
                    let _ = open::that_detached(format!("https://www.youtube.com/watch?v={id}"));
                }
            }
        }
    }

    /// Starts translating a description (once per project and session).
    pub fn translate_description(&mut self, key: &str, text: &str) {
        let Ok(mut map) = self.translations.lock() else {
            return;
        };
        if matches!(map.get(key), Some(Translation::Working | Translation::Done(..))) {
            self.show_translated.insert(key.to_string());
            return;
        }
        map.insert(key.to_string(), Translation::Working);
        drop(map);
        self.show_translated.insert(key.to_string());
        let (key, text, store, ctx) = (key.to_string(), text.to_string(), self.translations.clone(), self.ctx.clone());
        net::spawn(async move {
            let state = match crate::translate::to_russian(&net::client(), &text).await {
                Ok(t) => Translation::Done(t, false),
                Err(e) => Translation::Failed(format!("{e:#}")),
            };
            if let Ok(mut map) = store.lock() {
                map.insert(key, state);
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Current translation of a description; strips undrawable characters
    /// once, the first time the finished text is read.
    pub fn translation(&mut self, ctx: &egui::Context, key: &str) -> Option<Translation> {
        let mut map = self.translations.lock().ok()?;
        let entry = map.get_mut(key)?;
        if let Translation::Done(text, cleaned) = entry {
            if !*cleaned {
                *text = drawable_text(ctx, text);
                *cleaned = true;
            }
        }
        Some(entry.clone())
    }

    /// State of the «is this pack build current?» check; starts it once.
    pub fn pack_check(&mut self, profile: &crate::profiles::Profile) -> Option<PackCheck> {
        crate::mods::profile_hit(profile)?;
        let mut checks = self.pack_checks.lock().ok()?;
        if let Some(state) = checks.get(&profile.id) {
            return Some(state.clone());
        }
        checks.insert(profile.id.clone(), PackCheck::Checking);
        drop(checks);
        let (cfg, profile, store, ctx) = (self.cfg.clone(), profile.clone(), self.pack_checks.clone(), self.ctx.clone());
        net::spawn(async move {
            let state = match crate::mods::pack_update(&net::client(), &cfg, &profile).await {
                Ok(Some(newer)) => PackCheck::Update(newer),
                Ok(None) => PackCheck::Current,
                Err(_) => PackCheck::Failed,
            };
            if let Ok(mut checks) = store.lock() {
                checks.insert(profile.id.clone(), state);
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
        Some(PackCheck::Checking)
    }

    /// Reinstalls a pack build from the newest version of its pack: same
    /// build, worlds/screenshots/options kept (see `upsert_modpack_profile`).
    pub fn update_pack_build(&mut self, ctx: &egui::Context, profile: &crate::profiles::Profile) {
        let Some(hit) = crate::mods::profile_hit(profile) else {
            return;
        };
        if let Ok(mut checks) = self.pack_checks.lock() {
            checks.remove(&profile.id);
        }
        self.start_mod_install(ctx, hit);
    }

    /// The card answers straight from Modrinth/CurseForge: its download
    /// count and last update replace the (up to minutes old) search-index
    /// values in the grid and the page cache, so the tile matches the card.
    fn apply_fresh_detail(&mut self, d: &crate::mods::ProjectDetail) {
        let Some(open) = self.detail.as_mut() else {
            return;
        };
        if d.downloads > 0 {
            open.downloads = d.downloads;
        }
        if !d.updated.is_empty() {
            open.updated = d.updated.clone();
        }
        let (source, id) = (open.source.clone(), open.id.clone());
        let (downloads, updated) = (open.downloads, open.updated.clone());
        let lists = std::iter::once(&mut self.search.results).chain(self.search.cache.values_mut().map(|(_, v)| v));
        for list in lists {
            for h in list.iter_mut().filter(|h| h.source == source && h.id == id) {
                h.downloads = downloads;
                h.updated = updated.clone();
            }
        }
    }

    /// Opens the project card and loads its details. Opening another card
    /// replaces the channel, so a slow older answer can never show up here.
    pub fn open_detail(&mut self, hit: ModHit) {
        let spec = self.active_spec();
        let cfg = self.cfg.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.detail = Some(hit.clone());
        self.detail_at = Instant::now();
        self.detail_body = None;
        self.detail_rx = Some(rx);
        let ctx = self.ctx.clone();
        net::spawn(async move {
            let res = mods::detail(&net::client(), &cfg, &hit, &spec)
                .await
                .map_err(|e| format!("{e:#}"));
            let _ = tx.send(res);
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Pomme install: download the portable native client.
    pub fn start_pomme_install(&mut self, ctx: &egui::Context) {
        if self.pomme_task.as_ref().map(|t| !t.finished).unwrap_or(false) {
            self.toast("Pomme уже скачивается");
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<PommeEvent>();
        let ctx = ctx.clone();
        net::spawn(async move {
            let client = net::client();
            let mut send = |ev: PommeEvent| {
                let _ = tx.send(ev);
                ctx.request_repaint_after(Duration::from_millis(90));
            };
            if let Err(e) = pomme::install(&client, &mut send).await {
                send(PommeEvent::Error(format!("{e:#}")));
            }
        });
        self.pomme_task = Some(PommeTask {
            stage: "подготовка…".into(),
            done: 0,
            total: 0,
            finished: false,
            error: None,
            rx,
        });
    }

    pub fn launch_pomme(&mut self) {
        match pomme::launch(&self.shared, &self.cfg.nickname_or_default()) {
            Ok(Some(world)) => self.toast(format!("Pomme открывает мир «{world}»")),
            Ok(None) => self.toast("Pomme запускается — создай мир, дальше он будет открываться сразу"),
            Err(e) => self.toast(format!("{e:#}")),
        }
    }

    /// Cached [`crate::effects::inspect_instance_applied`] for the active build.
    pub fn effects_applied_state(&mut self) -> crate::effects::AppliedState {
        let instance = self.cfg.instance_dir();
        if let Some((dir, cfg, at, state)) = &self.effects_applied {
            if *dir == instance && *cfg == self.effects && at.elapsed() < Duration::from_secs(1) {
                return *state;
            }
        }
        let state = crate::effects::inspect_instance_applied(&instance, &self.effects);
        self.effects_applied = Some((instance, self.effects.clone(), Instant::now(), state));
        state
    }

    pub fn apply_effects_now(&mut self, notify: bool) {
        self.effects_applied = None;
        let instance = self.cfg.instance_dir();
        let spec = self.active_spec();
        let mc_running = self.auto_snap.mc_running || self.shared.is_game_running();
        self.fetch_missing_rp_set(&spec.mc);
        let jar = crate::effects::client_jar(&self.cfg, &spec.mc);
        match crate::effects::apply_to_instance(&instance, &spec.mc, &self.effects, jar.as_deref()) {
            Ok(rep) => {
                // Live: the running game reloads the pack (F3+T sent by the
                // launcher) the moment the player is back in it.
                let queued = rep.pack_written && mc_running;
                if queued {
                    self.automation.request_reload(&instance);
                }
                let msg = match (rep.pack_written, queued, mc_running, rep.options_updated) {
                    (true, true, _, _) => "Пак обновлён · включится, как только вернёшься в игру",
                    (true, false, _, _) => "Применено · включится при запуске игры",
                    (false, _, true, true) => "Гамма, тряска камеры и частицы — после перезапуска игры",
                    (false, _, false, true) => "Применено к options.txt",
                    (false, _, _, false) => "Всё уже применено",
                };
                self.effects_status = Some((true, msg.into()));
                if notify {
                    self.toast(msg);
                }
            }
            Err(e) => {
                let msg = format!("Не удалось применить эффекты: {e:#}");
                self.effects_status = Some((false, msg.clone()));
                if notify {
                    self.toast(msg);
                }
            }
        }
    }

    /// Downloads switched-on set packs that are not cached yet; when they
    /// arrive the effects are applied again (and the game reloads).
    fn fetch_missing_rp_set(&mut self, mc: &str) {
        if self.rp_fetch_rx.is_some() {
            return;
        }
        let missing: Vec<String> = self
            .effects
            .rp_set
            .iter()
            .filter(|s| !crate::effects::rp_cached(s, mc).is_file())
            .cloned()
            .collect();
        if missing.is_empty() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.rp_fetch_rx = Some(rx);
        let (mc, ctx) = (mc.to_string(), self.ctx.clone());
        net::spawn(async move {
            let res = crate::effects::fetch_set_packs(&net::client(), &mc, &missing).await.map_err(|e| format!("{e:#}"));
            let _ = tx.send(res);
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    pub fn start_effects_mod_install(&mut self, targets: Vec<crate::effects::EffectModSpec>) {
        if targets.is_empty() {
            self.toast("Подходящие моды уже установлены");
            return;
        }
        if self.effects_mod_task.as_ref().map(|t| !t.finished).unwrap_or(false) {
            self.toast("Скачивание модов эффектов уже идёт");
            return;
        }
        let Some(ctx) = self.ctx.clone() else {
            return;
        };
        let cfg = self.cfg.clone();
        let spec = self.active_spec();
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_bg = cancel.clone();
        net::spawn(async move {
            let client = net::client();
            let ctx_cb = ctx.clone();
            let tx_cb = tx.clone();
            let res = crate::effects::install_effect_mods(
                &client,
                &cfg,
                &spec,
                targets,
                &cancel_bg,
                move |stage| {
                    let _ = tx_cb.send(EffectsModMsg::Stage(stage));
                    ctx_cb.request_repaint();
                },
            )
            .await
            .map_err(|e| format!("{e:#}"));
            let _ = tx.send(EffectsModMsg::Finished(res));
            ctx.request_repaint();
        });
        self.effects_mod_task = Some(EffectsModTask {
            stage: "Подбор модов на Modrinth…".into(),
            finished: false,
            installed: Vec::new(),
            missing: Vec::new(),
            error: None,
            cancel,
            rx,
        });
    }

    pub fn start_mod_install(&mut self, ctx: &egui::Context, hit: ModHit) {
        if self.mod_task.as_ref().map(|t| !t.finished).unwrap_or(false) {
            self.toast("Уже скачивается");
            return;
        }
        let spec = self.active_spec();
        let cfg = self.cfg.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        let title = hit.title.clone();
        let kind = hit.kind.clone();
        if kind == mods::KIND_MODPACK {
            self.toast(format!("Ставлю модпак «{}» — своя сборка, всё само", hit.title));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = cancel.clone();
        let kind_task = kind.clone();
        net::spawn(async move {
            let client = net::client();
            let mut send = |ev: ModEvent| {
                let _ = tx.send(ev);
                ctx.request_repaint_after(Duration::from_millis(90));
            };
            if let Err(e) = mods::install(&client, &cfg, &hit, &spec, &kind_task, &cancel_task, &mut send).await {
                send(ModEvent::Error(format!("{e:#}")));
            }
        });
        self.mod_task = Some(ModTask {
            title,
            kind,
            done: 0,
            total: 0,
            finished: false,
            error: None,
            warn: None,
            path: None,
            cancel,
            last_file: String::new(),
            pack_meta: None,
            bytes: (0, 0),
            rx,
        });
    }

    /// «Играть» (optionally straight onto the hosted Madrid server).
    pub fn launch_game(&mut self, connect: bool) {
        let target = self.connect_target(connect);
        self.play(target);
    }

    /// Runs the version that fits the active build, joining `target` when
    /// given. Missing Minecraft, loader or Java are installed first, then
    /// the game opens by itself — one click, no setup screens.
    pub fn play(&mut self, target: Option<String>) {
        if self.shared.is_game_running() {
            self.toast("Игра уже запущена");
            return;
        }
        let spec = self.active_spec();
        // A Modrinth FTB pack installed earlier may still lack the FTB mods
        // (they are not on Modrinth): fetch them first, then start.
        let profile = crate::profiles::get_active_profile(&self.cfg);
        if profile.source.starts_with("modrinth:")
            && !self.ftb_tried.contains(&profile.id)
            && self.mod_task.as_ref().map(|t| t.finished).unwrap_or(true)
            && crate::ftb::needs_fill(&self.cfg.instance_dir())
        {
            self.ftb_tried.insert(profile.id.clone());
            self.start_ftb_fill(spec, target);
            return;
        }
        if let Some(t) = self.install.as_mut() {
            if !t.finished {
                if t.spec == spec {
                    t.auto_launch = true;
                    if target.is_some() {
                        self.pending_target = target;
                    }
                    self.toast("Ещё ставлю — игра откроется сама");
                } else {
                    self.toast("Дождись окончания установки");
                }
                return;
            }
        }
        match self.compatible_version() {
            Some(id) if launch::ready_to_launch(&self.cfg, &id) => {
                if self.cfg.selected_version != id {
                    self.cfg.selected_version = id;
                    self.save_cfg();
                }
                self.launch_to(target.as_deref());
            }
            _ => {
                let Some(ctx) = self.ctx.clone() else { return };
                let msg = format!("Готовлю {} — игра откроется сама", spec.title());
                self.toast(msg.clone());
                if target.is_some() {
                    self.friend_state = Some(FriendState::Working(msg));
                }
                self.pending_target = target;
                self.start_install(&ctx, spec, true);
            }
        }
    }

    /// The game stopped on «missing dependencies»: close it, fetch the
    /// missing mods for this build, start again.
    fn poll_missing_mods(&mut self) {
        let ready = match self.shared.missing_mods.lock() {
            Ok(guard) => guard
                .as_ref()
                .filter(|(_, _, at)| at.elapsed() >= Duration::from_millis(1500))
                .map(|(ids, pairs, _)| (ids.clone(), pairs.clone())),
            Err(_) => None,
        };
        let Some((ids, pairs)) = ready else { return };
        if let Ok(mut m) = self.shared.missing_mods.lock() {
            *m = None;
        }
        let mut key_ids = ids.clone();
        key_ids.sort();
        let key = format!("{}|{}", self.cfg.active_profile, key_ids.join(","));
        if !self.deps_tried.insert(key) || self.mod_task.as_ref().map(|t| !t.finished).unwrap_or(false) {
            self.toast(format!("Игре не хватает модов: {}", ids.join(", ")));
            return;
        }
        if self.shared.is_game_running() {
            launch::stop(&self.shared);
        }
        let Some(ctx) = self.ctx.clone() else { return };
        let cfg = self.cfg.clone();
        let spec = self.active_spec();
        let mods_dir = cfg.mods_dir();
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = cancel.clone();
        let ids_task = ids.clone();
        net::spawn(async move {
            let mut send = |ev: ModEvent| {
                let _ = tx.send(ev);
                ctx.request_repaint_after(Duration::from_millis(90));
            };
            match crate::deps::fetch_missing(&net::client(), &cfg, &ids_task, &pairs, &spec, &mods_dir, &cancel_task, &mut send).await {
                Ok((_added, missing)) if missing.is_empty() => send(ModEvent::Done(mods_dir.clone())),
                Ok((added, missing)) => {
                    if !added.is_empty() {
                        send(ModEvent::Warn(format!("не нашлись: {}", missing.join(", "))));
                        send(ModEvent::Done(mods_dir.clone()));
                    } else {
                        send(ModEvent::Error(format!("не нашлись моды: {}", missing.join(", "))));
                    }
                }
                Err(e) => send(ModEvent::Error(format!("{e:#}"))),
            }
        });
        self.toast(format!("Не хватает модов ({}) — докачиваю и перезапускаю", ids.len()));
        self.launch_after_mods = Some(None);
        self.mod_task = Some(ModTask {
            title: format!("Недостающие моды: {}", ids.join(", ")),
            kind: mods::KIND_MOD.into(),
            done: 0,
            total: ids.len() as u64,
            finished: false,
            error: None,
            warn: None,
            path: None,
            cancel,
            last_file: String::new(),
            pack_meta: None,
            bytes: (0, 0),
            rx,
        });
    }

    /// Downloads the FTB mods a Modrinth pack needs, then runs «Играть».
    fn start_ftb_fill(&mut self, spec: Spec, target: Option<String>) {
        let Some(ctx) = self.ctx.clone() else { return };
        let cfg = self.cfg.clone();
        let root = cfg.instance_dir();
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = cancel.clone();
        let ctx_task = ctx.clone();
        net::spawn(async move {
            let mut send = |ev: ModEvent| {
                let _ = tx.send(ev);
                ctx_task.request_repaint_after(Duration::from_millis(90));
            };
            match crate::ftb::fill_missing(&net::client(), &cfg, &root, &spec, &cancel_task, &mut send).await {
                Ok(_) => send(ModEvent::Done(root.join("mods"))),
                Err(e) => send(ModEvent::Error(format!("{e:#}"))),
            }
        });
        self.toast("Сборке не хватает FTB-модов — докачиваю с CurseForge");
        self.launch_after_mods = Some(target);
        self.mod_task = Some(ModTask {
            title: "FTB-моды для сборки".into(),
            kind: mods::KIND_MOD.into(),
            done: 0,
            total: 0,
            finished: false,
            error: None,
            warn: None,
            path: None,
            cancel,
            last_file: String::new(),
            pack_meta: None,
            bytes: (0, 0),
            rx,
        });
    }

    fn connect_target(&self, connect: bool) -> Option<String> {
        if connect && self.shared.is_mc_running() {
            Some(format!("127.0.0.1:{}", self.cfg.server_port))
        } else {
            None
        }
    }

    /// Launches the selected version, optionally joining `addr` directly.
    pub fn launch_to(&mut self, addr: Option<&str>) {
        let id = self.cfg.selected_version.clone();
        if id.is_empty() {
            self.play(addr.map(String::from));
            return;
        }
        match launch::launch(&self.shared, &self.cfg, &id, addr) {
            Ok(_) => {
                if let Some(a) = addr {
                    let msg = format!("Запускаю Minecraft и подключаюсь к {a}");
                    self.toast(msg.clone());
                    self.friend_state = Some(FriendState::Ok(msg));
                } else {
                    self.toast("Minecraft запускается");
                }
                self.was_running = true;
            }
            Err(e) => {
                let err = format!("Не удалось запустить игру: {e:#}");
                self.shared.push_game(format!("[Ошибка] {err}"));
                self.shared.log(LogKind::Error, &err);
                self.friend_state = Some(FriendState::Err(err.clone()));
                self.toast(err);
            }
        }
    }

    /// Removes every custom build and modpack (and loader versions only
    /// they used). The main Madrid build stays.
    pub fn delete_all_builds(&mut self) {
        let busy = self.install.as_ref().map(|t| !t.finished).unwrap_or(false)
            || self.mod_task.as_ref().map(|t| !t.finished).unwrap_or(false);
        if busy {
            self.toast("Дождись окончания загрузки");
            return;
        }
        if self.shared.is_game_running() {
            self.toast("Сначала закрой игру");
            return;
        }
        let mut cfg = self.cfg.clone();
        match crate::profiles::delete_all_builds(&mut cfg) {
            Ok(s) => {
                self.cfg = cfg;
                self.save_cfg();
                self.refresh_installed();
                self.touch_mods();
                self.storage_at = Instant::now() - Duration::from_secs(5);
                self.build_selected_id.clear();
                self.toast(format!("Удаляю сборок: {} · версий: {}", s.builds, s.versions));
            }
            Err(e) => self.toast(format!("Не удалось: {e:#}")),
        }
    }

    /// Public IP for the friend invite code, fetched once and cached.
    pub fn public_ip(&mut self, ctx: &egui::Context) -> Option<String> {
        if let Some(ip) = &self.public_ip_cached {
            return Some(ip.clone());
        }
        if !self.public_ip_fetching {
            self.public_ip_fetching = true;
            let ctx = ctx.clone();
            let home = self.lan_ip.clone();
            net::spawn(async move {
                if let Some(ip) = crate::upnp::public_ip(home).await {
                    let _ = PUBLIC_IP.set(ip);
                }
                ctx.request_repaint();
            });
        }
        if let Some((_, Ok(o))) = &self.upnp_state {
            if let Some(wan) = o.external_ip.as_ref().filter(|ip| !crate::upnp::is_unreachable_ip(ip)) {
                return Some(wan.clone());
            }
        }
        let ip = PUBLIC_IP.get().cloned();
        if let Some(ip) = &ip {
            self.public_ip_cached = Some(ip.clone());
        }
        ip
    }

    /// Cancels every in-flight download the user asked to stop.
    pub fn cancel_active_downloads(&mut self) {
        let mut any = false;
        if let Some(t) = &self.install {
            if !t.finished {
                t.cancel.store(true, Ordering::SeqCst);
                any = true;
            }
        }
        if let Some(t) = &self.mod_task {
            if !t.finished {
                t.cancel.store(true, Ordering::SeqCst);
                any = true;
            }
        }
        if let Some(t) = &self.build_task {
            if !t.finished {
                t.cancel.store(true, Ordering::SeqCst);
                any = true;
            }
        }
        if let Some(t) = &self.java_task {
            if !t.finished {
                t.cancel.store(true, Ordering::SeqCst);
                any = true;
            }
        }
        if any {
            self.toast("Отменяю…");
        }
    }

    pub fn stop_game(&mut self) {
        if launch::stop(&self.shared) {
            self.toast("Игра остановлена");
        }
    }

    // ------------------------------------------------------------------ polling

    pub fn poll(&mut self, ctx: &egui::Context) {
        let mut busy = false;

        let mut install_finished = false;
        if let Some(task) = self.install.as_mut() {
            let active = task.pump();
            if task.finished {
                install_finished = true;
            } else {
                busy |= active;
            }
        }
        if install_finished {
            if let Some(task) = self.install.take() {
                if task.ok {
                    self.refresh_installed();
                    self.toast(format!("Готово: {}", task.spec.title()));
                    if task.auto_launch && task.spec == self.active_spec() {
                        // Launch the exact id just installed — never loops
                        // back into another install if something is off.
                        self.cfg.selected_version = task.version.clone();
                        self.save_cfg();
                        let target = self.pending_target.take();
                        self.launch_to(target.as_deref());
                    }
                } else if task.error.as_deref().map(|e| e.contains("Отменено")).unwrap_or(false) {
                    self.toast("Установка отменена");
                    if self.pending_target.take().is_some() {
                        self.friend_state = Some(FriendState::Err("Установка отменена".into()));
                    }
                } else {
                    let msg = format!("Не установилось: {}", task.error.unwrap_or_default());
                    self.toast(msg.clone());
                    if self.pending_target.take().is_some() {
                        self.friend_state = Some(FriendState::Err(msg));
                    }
                }
            }
        }

        self.poll_remove(ctx);

        self.drain_search(ctx);
        // Dev screenshots: `MADRID_DEV_DETAIL=1` opens the first result's card.
        {
            static OPENED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if self.detail.is_none()
                && !self.search.results.is_empty()
                && std::env::var_os("MADRID_DEV_DETAIL").is_some()
                && !OPENED.swap(true, Ordering::Relaxed)
            {
                let hit = self.search.results[0].clone();
                self.open_detail(hit);
            }
            // `MADRID_DEV_VIDEO=1`: then plays the card's first video in the viewer.
            static PLAYED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if let (Some(hit), Some(Ok(d))) = (&self.detail, &self.detail_body) {
                if !d.videos.is_empty()
                    && std::env::var_os("MADRID_DEV_VIDEO").is_some()
                    && !PLAYED.swap(true, Ordering::Relaxed)
                {
                    let items = d.videos.iter().map(|v| crate::views::viewer::Media::Video(v.clone())).collect();
                    let title = hit.title.clone();
                    self.open_viewer(items, 0, &title);
                }
            }
        }
        if self.search.busy {
            busy = true;
        }
        self.drain_images(ctx);
        self.drain_pack_info();

        let mut mod_finished = false;
        let mut pack_meta: Option<(String, Spec)> = None;
        if let Some(task) = self.mod_task.as_mut() {
            let active = task.pump();
            pack_meta = task.pack_meta.take();
            if task.finished {
                mod_finished = true;
            } else {
                busy |= active;
            }
        }
        if let Some((profile_id, spec)) = pack_meta {
            let mut cfg = self.cfg.clone();
            if crate::profiles::switch_profile(&mut cfg, &profile_id).is_ok() {
                self.cfg = cfg;
                self.shared.set_cfg(self.cfg.clone());
                self.touch_mods();
            }
            // The pack's own Minecraft + loader download right away, in
            // parallel with its mods — «Играть» later just runs it.
            let installed = minecraft::as_installed(&self.installed());
            if crate::loader::pick_installed(&installed, &spec).is_none() {
                if let Some(ctx) = self.ctx.clone() {
                    self.start_install(&ctx, spec.clone(), false);
                }
            }
            self.toast(format!("Сборка пака · {}", spec.title()));
        }
        if mod_finished {
            if let Some(task) = self.mod_task.take() {
                self.touch_mods();
                if let Some(target) = self.launch_after_mods.take() {
                    if task.error.is_none() {
                        if let Some(w) = &task.warn {
                            self.toast(format!("Внимание: {w}"));
                        }
                        self.toast("Моды на месте — запускаю");
                        self.play(target);
                    } else {
                        self.toast(format!("Моды не скачались: {}", task.error.clone().unwrap_or_default()));
                    }
                } else if let Some(path) = task.path {
                    if task.kind == mods::KIND_MODPACK {
                        // The build now holds a new version: ask again.
                        if let Ok(mut checks) = self.pack_checks.lock() {
                            checks.clear();
                        }
                        self.toast(format!("Модпак «{}» готов — жми «Играть»", task.title));
                    } else {
                        self.toast(format!(
                            "Установлено: {}",
                            path.file_name().and_then(|s| s.to_str()).unwrap_or("")
                        ));
                    }
                    if let Some(w) = task.warn {
                        self.toast(format!("Внимание: {w}"));
                    }
                } else if task.error.as_deref().map(|e| e.contains("Отменено")).unwrap_or(false) {
                    self.toast("Скачивание отменено");
                } else {
                    self.toast(format!("Не скачалось: {}", task.error.unwrap_or_default()));
                }
            }
        }

        let mut java_finished = false;
        if let Some(task) = self.java_task.as_mut() {
            let active = task.pump();
            if task.finished {
                java_finished = true;
            } else {
                busy |= active;
            }
        }
        if java_finished {
            if let Some(task) = self.java_task.take() {
                if task.error.is_none() {
                    self.cfg.java_path = String::new();
                    self.reprobe_java();
                    self.toast(format!("Java {} установлена", task.major));
                    self.shared.log(LogKind::Info, format!("Java {} установлена", task.major));
                } else {
                    self.toast(format!("Java не установилась: {}", task.error.unwrap_or_default()));
                }
            }
        }

        if let Some(rx) = self.rp_fetch_rx.as_ref() {
            match rx.try_recv() {
                Ok(res) => {
                    self.rp_fetch_rx = None;
                    match res {
                        Ok(_) => self.apply_effects_now(false),
                        Err(e) => self.effects_status = Some((false, format!("Ресурспаки не скачались: {e}"))),
                    }
                }
                Err(TryRecvError::Empty) => busy = true,
                Err(TryRecvError::Disconnected) => self.rp_fetch_rx = None,
            }
        }

        if let Some(rx) = self.detail_rx.as_ref() {
            match rx.try_recv() {
                Ok(mut res) => {
                    if let Ok(d) = &mut res {
                        self.apply_fresh_detail(d);
                        // Emoji and symbols the fonts lack drew as «□».
                        d.body = drawable_text(ctx, &d.body);
                        d.summary = drawable_text(ctx, &d.summary);
                    }
                    self.detail_body = Some(res);
                    self.detail_rx = None;
                }
                Err(TryRecvError::Empty) => busy = true,
                Err(TryRecvError::Disconnected) => self.detail_rx = None,
            }
        }

        let mut build_finished = false;
        if let Some(task) = self.build_task.as_mut() {
            let active = task.pump();
            if task.finished {
                build_finished = true;
            } else {
                busy |= active;
            }
        }
        if build_finished {
            if let Some(task) = self.build_task.take() {
                match &task.error {
                    Some(e) => self.toast(format!("Сборка не встала: {e}")),
                    None => {
                        self.toast(format!("Сборка установлена · {} модов", task.done));
                        self.touch_mods();
                    }
                }
            }
        }

        let mut pomme_finished = false;
        if let Some(task) = self.pomme_task.as_mut() {
            let active = task.pump();
            if task.finished {
                pomme_finished = true;
            } else {
                busy |= active;
            }
        }
        if pomme_finished {
            if let Some(task) = self.pomme_task.take() {
                match &task.error {
                    Some(e) => self.toast(format!("Pomme не скачался: {e}")),
                    None => self.toast("Pomme готов — жми «Играть»"),
                }
            }
        }

        if let Some(rx) = self.update_rx.as_ref() {
            let answer = match rx.try_recv() {
                Ok(a) => Some(a),
                Err(TryRecvError::Disconnected) => Some(Err(())),
                Err(TryRecvError::Empty) => None,
            };
            if let Some(answer) = answer {
                self.update_rx = None;
                let manual = std::mem::take(&mut self.update_manual);
                match answer {
                    Ok(Some(info)) => {
                        self.update_status = UpdateStatus::Found(info.version.clone());
                        // A manual check shows even a version once hidden.
                        if manual || self.cfg.skip_update_version != info.version {
                            if manual {
                                self.update_dismissed = false;
                                if self.cfg.skip_update_version == info.version {
                                    self.cfg.skip_update_version.clear();
                                    self.save_cfg();
                                }
                            }
                            self.update_info = Some(info);
                            self.update_shown_at = Some(Instant::now());
                        }
                    }
                    Ok(None) => {
                        self.update_status = UpdateStatus::Latest;
                        if manual {
                            self.toast(format!("Установлена последняя версия {VERSION}"));
                        }
                    }
                    Err(()) => {
                        if manual {
                            self.update_status = UpdateStatus::Offline;
                            self.toast("Не удалось проверить обновления: нет связи с GitHub");
                        }
                    }
                }
            }
        }
        if self.update_info.is_none()
            && self.update_rx.is_none()
            && self.update_task.is_none()
            && self.update_checked_at.elapsed() > Duration::from_secs(6 * 3600)
        {
            self.check_update(ctx);
        }
        // Idle windows still wake for the next check (no busy repaint).
        ctx.request_repaint_after(Duration::from_secs(6 * 3600));
        let mut update_ready: Option<PathBuf> = None;
        let mut update_err: Option<String> = None;
        if let Some(task) = self.update_task.as_mut() {
            let active = task.pump();
            if task.finished {
                update_ready = task.ready_path.clone();
                update_err = task.error.clone();
            } else {
                busy |= active;
            }
        }
        if let Some(installer) = update_ready {
            self.update_task = None;
            if let Ok(cur_exe) = std::env::current_exe() {
                match crate::updater::launch_installer_update(&installer, &cur_exe) {
                    Ok(_) => {
                        // A normal close: main() stops the hosted server and
                        // on_exit releases the UPnP port (exit(0) skipped both
                        // and left a Java server running). The installer waits
                        // for the exe to unlock.
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    Err(e) => self.toast(format!("{e:#}")),
                }
            } else {
                self.toast("Не найден путь текущего exe для обновления");
            }
        } else if let Some(err) = update_err {
            if err.contains("Отменено") {
                self.update_task = None;
                self.toast("Обновление отменено");
            }
        }

        let running = self.shared.is_game_running();
        if self.was_running && !running {
            self.toast("Игра завершилась");
            if self.minimized_for_game {
                self.minimized_for_game = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
        if !self.was_running && running {
            // The game gets the GPU and CPU to itself: the launcher steps
            // aside (and comes back when the game closes).
            self.minimized_for_game = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            crate::util::trim_memory();
        }
        self.was_running = running;

        let mc_running = self.shared.is_mc_running();
        if self.was_mc_running && !mc_running {
            self.toast("Сервер остановлен");
        }
        if !self.was_mc_running && mc_running {
            let port = self.shared.mc_port();
            self.toast(format!("Сервер Мадрид работает · 127.0.0.1:{port}"));
        }
        self.was_mc_running = mc_running;

        let mut eff_mod_done = false;
        if let Some(task) = self.effects_mod_task.as_mut() {
            let active = task.pump();
            if task.finished {
                eff_mod_done = true;
            } else {
                busy |= active;
            }
        }
        if eff_mod_done {
            if let Some(task) = self.effects_mod_task.take() {
                self.touch_mods();
                if let Some(err) = task.error {
                    self.effects_status = Some((false, err.clone()));
                    self.toast(err);
                } else if !task.installed.is_empty() {
                    let restart_note = if self.auto_snap.mc_running {
                        " · Перезапусти Minecraft для включения новых модов"
                    } else {
                        ""
                    };
                    let mut msg = format!("Моды установлены: {}{}", task.installed.join(", "), restart_note);
                    if !task.missing.is_empty() {
                        msg.push_str(&format!(" (не найдено: {})", task.missing.len()));
                    }
                    self.effects_status = Some((true, msg.clone()));
                    self.toast(msg);
                } else if !task.missing.is_empty() {
                    let msg = format!("Не нашлись версии для сборки: {}", task.missing.join(", "));
                    self.effects_status = Some((false, msg.clone()));
                    self.toast(msg);
                }
            }
        }

        // Automation hotkey / live Minecraft status sync
        self.auto_snap = self.automation.snapshot();
        if self.auto_snap.reload_seq != self.auto_seen_reload_seq {
            self.auto_seen_reload_seq = self.auto_snap.reload_seq;
            self.effects_status = Some((true, "Применено в игре · ресурсы перезагружены".into()));
        }
        if self.auto_snap.toggle_seq != self.auto_seen_toggle_seq {
            self.auto_seen_toggle_seq = self.auto_snap.toggle_seq;
            let mut changed = false;
            if self.effects.autoclicker != self.auto_snap.autoclicker {
                self.effects.autoclicker = self.auto_snap.autoclicker;
                changed = true;
                self.toast(if self.effects.autoclicker {
                    "Автокликер включён (работает в Minecraft)"
                } else {
                    "Автокликер выключен"
                });
            }
            if self.effects.anti_afk != self.auto_snap.anti_afk {
                self.effects.anti_afk = self.auto_snap.anti_afk;
                changed = true;
                self.toast(if self.effects.anti_afk {
                    "Защита от АФК включена"
                } else {
                    "Защита от АФК выключена"
                });
            }
            if changed {
                crate::effects::save(&self.effects);
            }
        }

        if let Some(target) = self.bind_capture {
            ctx.request_repaint_after(Duration::from_millis(30));
            if let Some(cap) = crate::automation::poll_new_bind() {
                match cap {
                    crate::automation::BindCapture::Cancel => {
                        self.bind_capture = None;
                        self.automation.set_capturing_bind(false);
                    }
                    crate::automation::BindCapture::Clear => {
                        match target {
                            crate::automation::BindTarget::Autoclicker => self.effects.autoclicker_bind.clear(),
                            crate::automation::BindTarget::AntiAfk => self.effects.anti_afk_bind.clear(),
                        }
                        crate::effects::save(&self.effects);
                        self.automation.update_from_config(&self.effects);
                        self.bind_capture = None;
                        self.automation.set_capturing_bind(false);
                        self.toast("Бинд очищен");
                    }
                    crate::automation::BindCapture::Set(key) => {
                        let nice = crate::automation::bind_display(&key);
                        match target {
                            crate::automation::BindTarget::Autoclicker => self.effects.autoclicker_bind = key,
                            crate::automation::BindTarget::AntiAfk => self.effects.anti_afk_bind = key,
                        }
                        crate::effects::save(&self.effects);
                        self.automation.update_from_config(&self.effects);
                        self.bind_capture = None;
                        self.automation.set_capturing_bind(false);
                        self.toast(format!("Бинд установлен: {nice}"));
                    }
                }
            }
        }

        if self.tab == Tab::Effects && (self.effects.anti_afk || self.effects.autoclicker) && self.auto_snap.mc_running {
            ctx.request_repaint_after(Duration::from_millis(500));
        }

        self.refresh_caches();
        self.poll_room(ctx);
        self.poll_missing_mods();
        if self.shared.missing_mods.lock().map(|m| m.is_some()).unwrap_or(false) {
            ctx.request_repaint_after(Duration::from_millis(500));
        }

        // Speed meter runs only while something downloads.
        let downloading = self.install.is_some()
            || self.mod_task.as_ref().map(|t| !t.finished).unwrap_or(false)
            || self.java_task.is_some()
            || self.build_task.as_ref().map(|t| !t.finished).unwrap_or(false)
            || self.pomme_task.is_some();
        if downloading {
            self.speed.tick();
        } else if self.speed.bps > 0.0 {
            self.speed = net::SpeedMeter::default();
        }

        // Repaint only as often as something visibly changes: downloads at
        // ~8 fps, a running game/server (logs) at 2 fps, idle not at all.
        // A hidden or unfocused launcher behind a running game draws
        // nothing: every frame here is a frame the game loses.
        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        let visible = ctx.input(|i| i.focused) && !minimized;
        // Idle pages go back to Windows once after start-up and whenever
        // the window is minimised — the launcher sits at a few MB then.
        if (minimized && !self.trimmed) || (!self.trimmed_start && self.started.elapsed() > Duration::from_secs(5)) {
            self.trimmed = minimized;
            self.trimmed_start = true;
            crate::util::trim_memory();
        }
        if !minimized {
            self.trimmed = false;
        }
        // Alt-Tab away: give idle pages back too (at most twice a minute).
        let focused = ctx.input(|i| i.focused);
        if self.was_focused && !focused && self.last_trim.elapsed() > Duration::from_secs(30) {
            self.last_trim = Instant::now();
            crate::util::trim_memory();
        }
        self.was_focused = focused;
        if !self.trimmed_start {
            ctx.request_repaint_after(Duration::from_secs(6));
        }
        if busy {
            ctx.request_repaint_after(Duration::from_millis(if visible { 120 } else { 1000 }));
        } else if visible && (running || mc_running || self.shared.is_mc_busy()) {
            ctx.request_repaint_after(Duration::from_millis(500));
        } else if running || mc_running {
            ctx.request_repaint_after(Duration::from_secs(3));
        }
        self.toasts.retain(|t| t.at.elapsed().as_secs_f32() < 5.0);
        if !self.toasts.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    // ------------------------------------------------------------------ rooms

    /// What this launcher announces to a room right now.
    fn my_presence(&mut self) -> crate::room::Presence {
        if self.cfg.device_id.len() < 8 {
            self.cfg.device_id = crate::room::device_id(&self.cfg.device_id);
            self.save_cfg();
        }
        let public = self.ctx.clone().and_then(|c| self.public_ip(&c)).unwrap_or_default();
        let hosting = self.room.as_ref().map(|r| r.me().host).unwrap_or(false);
        crate::room::Presence {
            v: 1,
            id: self.cfg.device_id.clone(),
            nick: self.cfg.nickname_or_default(),
            lan: self.lan_ip.clone().unwrap_or_default(),
            public,
            port: self.cfg.host_port,
            host: hosting,
            mc: self.active_spec().title(),
            leave: false,
        }
    }

    pub fn room_join(&mut self, code: &str) {
        let Some(code) = crate::room::parse_code(code) else {
            let msg = "Код комнаты должен состоять из 5 цифр".to_string();
            self.friend_state = Some(FriendState::Err(msg.clone()));
            self.toast(msg);
            return;
        };
        self.room_leave();
        let Some(ctx) = self.ctx.clone() else { return };
        let me = self.my_presence();
        self.room = Some(crate::room::Room::join(&code, me, ctx));
        self.cfg.last_room = code.clone();
        self.save_cfg();
        self.friend_state = Some(FriendState::Working(format!("Подключаюсь к комнате {code}…")));
        self.toast(format!("Ты в комнате {code}"));
    }

    pub fn room_create(&mut self) {
        let code = crate::room::new_code();
        self.room_join(&code);
        if let Some(ctx) = self.ctx.clone() {
            crate::util::copy_text(&ctx, &code);
        }
        self.friend_state = Some(FriendState::Ok(format!("Комната {code} создана · код скопирован")));
        self.toast(format!("Комната {code} создана · код скопирован"));
    }

    pub fn room_leave(&mut self) {
        self.set_hosting(false);
        if let Some(room) = self.room.take() {
            room.leave(false);
        }
    }

    /// Starts/stops announcing our game server; opens the router port.
    pub fn set_hosting(&mut self, on: bool) {
        let port = self.cfg.host_port;
        if let Some(room) = &self.room {
            room.update_me(|p| {
                p.host = on;
                p.port = port;
            });
        }
        if on {
            self.open_router_port(port);
        } else if let Some((mapped, Ok(_))) = self.upnp_state.take() {
            let lan = self.lan_ip.clone();
            net::spawn(async move {
                let _ = crate::upnp::close(&net::client(), mapped, lan).await;
            });
        }
    }

    pub fn open_router_port(&mut self, port: u16) {
        if matches!(&self.upnp_state, Some((p, Ok(_))) if *p == port) {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let lan = self.lan_ip.clone();
        let ctx = self.ctx.clone();
        net::spawn(async move {
            let res = crate::upnp::open(&net::client(), port, lan).await.map_err(|e| format!("{e:#}"));
            let _ = tx.send(res);
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
        self.upnp_rx = Some((port, rx));
        self.upnp_state = None;
    }

    fn poll_room(&mut self, ctx: &egui::Context) {
        if let Some((port, rx)) = &self.upnp_rx {
            match rx.try_recv() {
                Ok(res) => {
                    self.upnp_state = Some((*port, res));
                    self.upnp_rx = None;
                }
                Err(TryRecvError::Disconnected) => self.upnp_rx = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        if self.room.is_some() {
            // Public IP / nickname / build may change while in the room.
            let fresh = self.my_presence();
            if let Some(room) = &self.room {
                room.update_me(|p| {
                    p.nick = fresh.nick;
                    p.lan = fresh.lan;
                    if !fresh.public.is_empty() {
                        p.public = fresh.public;
                    }
                    p.mc = fresh.mc;
                });
            }
            ctx.request_repaint_after(Duration::from_secs(2));
        }
    }

    /// Joins a friend's server with the active build.
    pub fn join_address(&mut self, addr: &str) {
        let trimmed = addr.trim();
        let ok = trimmed
            .rsplit_once(':')
            .map(|(ip, port)| !ip.trim().is_empty() && port.trim().parse::<u16>().map(|p| p > 0).unwrap_or(false))
            .unwrap_or(false);
        if !ok {
            let msg = "Неверный адрес: нужен формат IP:порт (например 192.168.1.5:25565)".to_string();
            self.friend_state = Some(FriendState::Err(msg.clone()));
            self.toast(msg);
            return;
        }
        if self.shared.is_game_running() {
            let msg = "Игра уже запущена — сначала закрой её".to_string();
            self.friend_state = Some(FriendState::Err(msg.clone()));
            self.toast(msg);
            return;
        }
        self.friend_state = Some(FriendState::Working(format!("Запускаю игру и подключаюсь к {trimmed}…")));
        self.play(Some(trimmed.to_string()));
    }

    /// Window closes: tell the room we left, close the router port.
    pub fn shutdown_online(&mut self) {
        if let Some(room) = self.room.take() {
            room.leave(true);
        }
        if let Some((port, Ok(_))) = self.upnp_state.take() {
            let lan = self.lan_ip.clone();
            let _ = net::rt().block_on(async {
                tokio::time::timeout(
                    Duration::from_secs(3),
                    crate::upnp::close(&net::client(), port, lan),
                )
                .await
            });
        }
    }

    // ------------------------------------------------------------------ chrome

    fn titlebar(&mut self, ui: &mut Ui, skin: &Skin) {
        let rect = ui.max_rect();
        let cy = rect.center().y;

        let drag = Rect::from_min_max(rect.min, Pos2::new(rect.right() - 178.0, rect.bottom()));
        let drag_resp = ui.interact(drag, Id::unique("title-drag"), Sense::click_and_drag());
        if drag_resp.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag_resp.double_clicked() {
            let maxed = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(!maxed));
        }

        let painter = ui.painter();
        // Wordmark only: the name, no logo tile.
        let mut x = rect.left() + 20.0;
        painter.text(
            Pos2::new(x, cy),
            Align2::LEFT_CENTER,
            "SAD LAUNCHER",
            theme::display(13.5),
            skin.ink(),
        );
        x += painter
            .layout_no_wrap(
                "SAD LAUNCHER".into(),
                theme::display(13.5),
                skin.ink(),
            )
            .size()
            .x
            + 10.0;
        painter.line_segment(
            [Pos2::new(x, cy - 6.0), Pos2::new(x, cy + 6.0)],
            Stroke::new(1.0, skin.line()),
        );
        x += 10.0;
        painter.text(
            Pos2::new(x, cy),
            Align2::LEFT_CENTER,
            &format!("MINECRAFT · v{}", crate::config::VERSION),
            theme::mono(9.5),
            skin.faint(),
        );

        let btn_w = 44.0;
        let close = Rect::from_min_max(
            Pos2::new(rect.right() - btn_w, rect.top()),
            Pos2::new(rect.right(), rect.bottom()),
        );
        let maxi = close.translate(Vec2::new(-btn_w, 0.0));
        let mini = maxi.translate(Vec2::new(-btn_w, 0.0));

        self.window_button(ui, skin, close, Icon::Close, true);
        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        self.window_button(
            ui,
            skin,
            maxi,
            if maximized {
                Icon::Restore
            } else {
                Icon::Maximize
            },
            false,
        );
        self.window_button(ui, skin, mini, Icon::Minimize, false);

        let mc_port = self.shared.mc_port();
        let label = if mc_port > 0 {
            format!("СЕРВЕР · 127.0.0.1:{mc_port}")
        } else {
            let stage = self.shared.mc_stage();
            if stage.is_empty() {
                "СЕРВЕР · ГОТОВИТСЯ".to_string()
            } else {
                format!("СЕРВЕР · {}", stage.to_uppercase())
            }
        };
        let galley = ui
            .painter()
            .layout_no_wrap(label.clone(), theme::mono(10.0), skin.dim());
        let pill_w = galley.size().x + 40.0;
        let pill = Rect::from_min_size(
            Pos2::new(mini.left() - pill_w - 10.0, cy - 13.0),
            Vec2::new(pill_w, 26.0),
        );
        let resp = ui.interact(pill, Id::unique("server-pill"), Sense::click());
        if resp.hovered() {
            ui.painter()
                .rect_filled(pill, CornerRadius::same(13), skin.hover());
        }
        let online = mc_port > 0;
        ui.painter().circle_filled(
            Pos2::new(pill.left() + 14.0, cy),
            3.0,
            if online { skin.green() } else { skin.faint() },
        );
        ui.painter().text(
            Pos2::new(pill.left() + 24.0, cy),
            Align2::LEFT_CENTER,
            &label,
            theme::mono(10.0),
            skin.dim(),
        );
        if resp.clicked() {
            self.tab = Tab::Server;
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
    }

    fn window_button(&self, ui: &mut Ui, skin: &Skin, rect: Rect, icon: Icon, is_close: bool) {
        let resp = ui.interact(rect, Id::unique(("winbtn", icon as u8)), Sense::click());
        let hover = ui
            .ctx()
            .animate_bool_with_time(resp.id, resp.hovered(), 0.12);
        if hover > 0.01 {
            let fill = if is_close {
                Color32::from_rgba_unmultiplied(255, 255, 255, (238.0 * hover) as u8)
            } else {
                skin.hover().gamma_multiply(hover)
            };
            ui.painter()
                .rect_filled(rect.shrink(4.0), CornerRadius::same(7), fill);
        }
        let color = if is_close && hover > 0.5 {
            skin.black()
        } else {
            theme::lerp_color(skin.dim(), skin.ink(), hover)
        };
        icons::paint(
            ui.painter(),
            Rect::from_center_size(rect.center(), Vec2::splat(11.0)),
            icon,
            color,
            1.4,
        );
        if resp.clicked() {
            match icon {
                Icon::Close => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                Icon::Minimize => ui
                    .ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
                _ => {
                    let maxed = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!maxed));
                }
            }
        }
    }

    fn rail(&mut self, ui: &mut Ui, skin: &Skin) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(
            rect,
            CornerRadius {
                nw: 0,
                ne: 0,
                sw: 18,
                se: 0,
            },
            Color32::from_rgba_unmultiplied(255, 255, 255, 5),
        );
        widgets::vline(
            ui.painter(),
            rect.right() - 0.5,
            rect.top(),
            rect.bottom(),
            skin.line_soft(),
        );

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.label(
                egui::RichText::new("РАЗДЕЛЫ")
                    .font(theme::mono(9.0))
                    .color(skin.faint()),
            );
        });
        ui.add_space(8.0);

        let mut go: Option<Tab> = None;
        // Lay the items out first, then slide one highlight between them:
        // the selection travels instead of blinking from row to row.
        let rows: Vec<(Tab, Rect, egui::Response)> = Tab::all()
            .into_iter()
            .map(|tab| {
                let (rect, resp) = ui
                    .allocate_exact_size(Vec2::new(ui.available_width() - 24.0, 38.0), Sense::click());
                (tab, rect.translate(Vec2::new(12.0, 0.0)), resp)
            })
            .collect();
        if let Some((_, active_rect, _)) = rows.iter().find(|(t, _, _)| *t == self.tab) {
            let y = theme::tween(ui.ctx(), Id::unique("rail-highlight"), active_rect.top(), 0.26);
            let hl = Rect::from_min_size(Pos2::new(active_rect.left(), y), active_rect.size());
            ui.painter().rect_filled(hl, CornerRadius::same(10), skin.fill());
            ui.painter().rect_filled(
                Rect::from_center_size(Pos2::new(hl.left() + 2.0, hl.center().y), Vec2::new(2.0, 16.0)),
                CornerRadius::same(1),
                skin.ink(),
            );
        }
        for (tab, rect, resp) in rows {
            let active = self.tab == tab;
            let hover = ui
                .ctx()
                .animate_bool_with_time(resp.id, resp.hovered(), 0.12);
            let lit = ui.ctx().animate_bool_with_time(resp.id.with("lit"), active, 0.2);
            if !active && hover > 0.01 {
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::same(10),
                    skin.hover().gamma_multiply(hover),
                );
            }
            let tint = theme::lerp_color(skin.dim(), skin.ink(), hover.max(lit));
            icons::paint(
                ui.painter(),
                Rect::from_center_size(
                    Pos2::new(rect.left() + 25.0, rect.center().y),
                    Vec2::splat(17.0),
                ),
                tab.icon(),
                tint,
                1.5,
            );
            ui.painter().text(
                Pos2::new(rect.left() + 46.0, rect.center().y),
                Align2::LEFT_CENTER,
                tab.label(),
                theme::body(13.5),
                tint,
            );
            if resp.clicked() {
                go = Some(tab);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
        }
        if let Some(t) = go {
            if self.tab == t && t == Tab::Mods {
                // A second click on «Моды» closes an open project card.
                self.detail = None;
            }
            self.go(t);
        }

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.label(
                egui::RichText::new("ПРОФИЛЬ")
                    .font(theme::mono(9.0))
                    .color(skin.faint()),
            );
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            let java_ok = self.shared.java().is_some();
            let face = self.skin_face(ui.ctx()).cloned();
            let (slot, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
            if let Some(tex) = &face {
                ui.painter().image(
                    tex.id(),
                    slot,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
                ui.painter().rect_stroke(
                    slot,
                    CornerRadius::same(6),
                    Stroke::new(1.0, skin.line()),
                    egui::StrokeKind::Inside,
                );
            } else {
                widgets::paint_status_dot(
                    ui.painter(),
                    slot.center(),
                    if java_ok { skin.green() } else { skin.red() },
                    java_ok,
                );
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(
                    egui::RichText::new(self.cfg.nickname_or_default())
                        .font(theme::body(12.5))
                        .color(skin.ink()),
                );
                ui.label(
                    egui::RichText::new(if java_ok { "Java найдена" } else { "Java не найдена" })
                        .font(theme::mono(9.0))
                        .color(if java_ok { skin.faint() } else { skin.red() }),
                );
            });
        });

        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.add_space(14.0);
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), 24.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new(format!("v{VERSION}"))
                            .font(theme::mono(10.0))
                            .color(skin.faint().gamma_multiply(0.9)),
                    );
                    ui.add_space(6.0);
                    if widgets::button_sized(ui, skin, "О лаунчере", widgets::Btn::Ghost, true, 24.0, 11.0, 9.0)
                        .on_hover_text("Версия, папки, сайт и подсказки")
                        .clicked()
                    {
                        self.about_at = Some(Instant::now());
                    }
                    if !config::SITE_URL.is_empty()
                        && widgets::button_sized(ui, skin, "Сайт", widgets::Btn::Ghost, true, 24.0, 11.0, 9.0)
                            .on_hover_text(config::SITE_URL)
                            .clicked()
                    {
                        let _ = open::that_detached(config::SITE_URL);
                    }
                },
            );
            ui.add_space(10.0);
            let mc_port = self.shared.mc_port();
            let (rect, resp) = ui.allocate_exact_size(
                Vec2::new(ui.available_width() - 24.0, 52.0),
                Sense::click(),
            );
            let rect = rect.translate(Vec2::new(12.0, 0.0));
            let hover = ui
                .ctx()
                .animate_bool_with_time(resp.id, resp.hovered(), 0.12);
            if hover > 0.01 {
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::same(10),
                    skin.hover().gamma_multiply(hover),
                );
            }
            ui.painter().text(
                Pos2::new(rect.left() + 14.0, rect.top() + 14.0),
                Align2::LEFT_CENTER,
                "ЛОКАЛЬНЫЙ СЕРВЕР",
                theme::mono(8.5),
                skin.faint(),
            );
            let label = if mc_port > 0 {
                format!("127.0.0.1:{mc_port}")
            } else {
                let stage = self.shared.mc_stage();
                if stage.is_empty() {
                    "готовится…".to_string()
                } else {
                    stage
                }
            };
            ui.painter().text(
                Pos2::new(rect.left() + 14.0, rect.top() + 33.0),
                Align2::LEFT_CENTER,
                &label,
                theme::mono(11.0),
                skin.ink(),
            );
            ui.painter().circle_filled(
                Pos2::new(rect.right() - 16.0, rect.top() + 14.0),
                2.5,
                if mc_port > 0 { skin.green() } else { skin.faint() },
            );
            if resp.clicked() {
                self.go(Tab::Server);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
        });
    }

    /// First-run screen: create the nickname that shows up in Minecraft.
    fn nickname_screen(&mut self, ui: &mut Ui, skin: &Skin) {
        if self.cfg.first_run_done {
            return;
        }
        let full = ui.max_rect();
        ui.painter()
            .rect_filled(full, CornerRadius::same(18), Color32::from_rgba_unmultiplied(8, 8, 10, 252));

        let size = Vec2::new(480.0, 260.0);
        let rect = Rect::from_center_size(full.center(), size);
        let mut panel = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::top_down(Align::Center)),
        );
        panel.add_space(34.0);
        panel.label(
            egui::RichText::new("Как тебя звать в Майнкрафте?")
                .font(theme::display(22.0))
                .color(skin.ink()),
        );
        panel.add_space(6.0);
        panel.label(
            egui::RichText::new("Ник увидят на сервере Мадрид и другие игроки")
                .font(theme::body(12.5))
                .color(skin.dim()),
        );
        panel.add_space(22.0);

        let field_w = 320.0;
        let (frect, _) = panel.allocate_exact_size(Vec2::new(field_w, 40.0), Sense::hover());
        panel.painter()
            .rect_filled(frect, CornerRadius::same(10), skin.fill());
        panel.painter().rect_stroke(
            frect,
            CornerRadius::same(10),
            Stroke::new(1.0, skin.line()),
            egui::StrokeKind::Inside,
        );
        let mut field = panel.new_child(
            egui::UiBuilder::new()
                .max_rect(frect.shrink2(Vec2::new(12.0, 8.0)))
                .layout(Layout::left_to_right(Align::Center)),
        );
        field.spacing_mut().item_spacing = Vec2::ZERO;
        let resp = field.add(
            egui::TextEdit::singleline(&mut self.nick_buf)
                .id_salt("nick-create")
                .hint_text(RichText::new("Никнейм, 3–16 символов").color(skin.faint()))
                .desired_width(f32::INFINITY)
                .font(theme::body(15.0))
                .text_color(skin.ink())
                .margin(egui::Margin::same(0))
                .frame(egui::Frame::NONE),
        );
        resp.request_focus();

        panel.add_space(18.0);
        let nick = self.nick_buf.trim().to_string();
        let ok = crate::util::nickname_valid(&nick);
        let label = if ok { "Поехали" } else { "Придумай ник" };
        let (brect, bresp) = panel.allocate_exact_size(Vec2::new(field_w, 44.0), Sense::click());
        let bg = if ok {
            Color32::from_rgba_unmultiplied(255, 255, 255, 24)
        } else {
            skin.fill()
        };
        let border = if ok {
            Color32::from_rgba_unmultiplied(255, 255, 255, 130)
        } else {
            skin.line_soft()
        };
        panel.painter().rect_filled(brect, CornerRadius::same(12), bg);
        panel.painter().rect_stroke(brect, CornerRadius::same(12), Stroke::new(1.0, border), egui::StrokeKind::Inside);
        panel.painter().text(
            brect.center(),
            Align2::CENTER_CENTER,
            label,
            theme::display(14.5),
            if ok { skin.ink() } else { skin.faint() },
        );
        if bresp.hovered() && ok {
            panel.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let enter = panel.input(|i| i.key_pressed(egui::Key::Enter));
        if ok && (bresp.clicked() || enter) {
            self.cfg.nickname = nick;
            self.cfg.first_run_done = true;
            self.save_cfg();
            self.toast(format!("Привет, {}", self.cfg.nickname_or_default()));
        }
    }

    /// Frameless windows get invisible resize edges — the native ones are gone.
    fn resize_edges(&mut self, ui: &mut Ui) {
        use egui::ResizeDirection;
        let rect = ui.max_rect();
        const T: f32 = 7.0;
        let zones: [(ResizeDirection, Rect, egui::CursorIcon); 8] = [
            (
                ResizeDirection::North,
                Rect::from_min_size(rect.left_top(), Vec2::new(rect.width(), T)),
                CursorIcon::ResizeNorth,
            ),
            (
                ResizeDirection::South,
                Rect::from_min_size(rect.left_bottom(), Vec2::new(rect.width(), T)),
                CursorIcon::ResizeSouth,
            ),
            (
                ResizeDirection::West,
                Rect::from_min_size(rect.left_top(), Vec2::new(T, rect.height())),
                CursorIcon::ResizeWest,
            ),
            (
                ResizeDirection::East,
                Rect::from_min_size(
                    Pos2::new(rect.right() - T, rect.top()),
                    Vec2::new(T, rect.height()),
                ),
                CursorIcon::ResizeEast,
            ),
            (
                ResizeDirection::NorthWest,
                Rect::from_min_size(rect.left_top(), Vec2::splat(T * 2.0)),
                CursorIcon::ResizeNorthWest,
            ),
            (
                ResizeDirection::NorthEast,
                Rect::from_min_size(
                    Pos2::new(rect.right() - T * 2.0, rect.top()),
                    Vec2::splat(T * 2.0),
                ),
                CursorIcon::ResizeNorthEast,
            ),
            (
                ResizeDirection::SouthWest,
                Rect::from_min_size(
                    Pos2::new(rect.left(), rect.bottom() - T * 2.0),
                    Vec2::splat(T * 2.0),
                ),
                CursorIcon::ResizeSouthWest,
            ),
            (
                ResizeDirection::SouthEast,
                Rect::from_min_size(
                    Pos2::new(rect.right() - T * 2.0, rect.bottom() - T * 2.0),
                    Vec2::splat(T * 2.0),
                ),
                CursorIcon::ResizeSouthEast,
            ),
        ];
        for (dir, zone, icon) in zones {
            let resp = ui.interact(zone, Id::unique(("resize", dir as u8)), Sense::click_and_drag());
            if resp.hovered() {
                ui.ctx().set_cursor_icon(icon);
            }
            if resp.drag_started() {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
            }
        }
    }

    /// Follows the background remover: refreshes lists after each removed
    /// item and keeps the progress card's state.
    fn poll_remove(&mut self, ctx: &egui::Context) {
        let seen = crate::remove::finished_count();
        if seen != self.remove_seen {
            self.remove_seen = seen;
            self.refresh_installed();
            self.touch_mods();
            self.storage_at = Instant::now() - Duration::from_secs(5);
        }
        match crate::remove::progress() {
            Some(p) => {
                match self.remove_card.as_mut() {
                    Some(card) if card.done_at.is_none() => card.last = p,
                    _ => {
                        self.remove_card = Some(RemoveCard {
                            last: p,
                            shown_at: Instant::now(),
                            done_at: None,
                            freed: 0,
                        })
                    }
                }
                ctx.request_repaint_after(Duration::from_millis(60));
            }
            None => {
                if let Some(card) = self.remove_card.as_mut() {
                    if card.done_at.is_none() {
                        card.done_at = Some(Instant::now());
                        card.freed = crate::remove::last_freed();
                        card.last.done_files = card.last.total_files;
                    }
                    if card.done_at.map(|t| t.elapsed().as_secs_f32() > 2.4).unwrap_or(false) {
                        self.remove_card = None;
                    } else {
                        ctx.request_repaint_after(Duration::from_millis(16));
                    }
                }
            }
        }
    }

    /// Bottom-right card: «Удаление · Сборка X», bar, files and freed space.
    /// Pops in with a scale from 92 %, shrinks away after «Удалено».
    fn draw_remove_card(&self, ctx: &egui::Context, skin: &Skin) {
        let Some(card) = &self.remove_card else { return };
        let enter = theme::ease_out(card.shown_at.elapsed().as_secs_f32() / 0.26);
        let exit = card
            .done_at
            .map(|t| 1.0 - theme::ease_out(((t.elapsed().as_secs_f32() - 1.8) / 0.5).max(0.0)))
            .unwrap_or(1.0);
        let vis = enter * exit;
        if vis <= 0.001 {
            return;
        }
        let layer = egui::LayerId::new(egui::Order::Foreground, Id::unique("remove-card"));
        let screen = ctx.content_rect();
        let size = Vec2::new(300.0, 86.0);
        let rect = Rect::from_min_size(
            Pos2::new(screen.right() - size.x - 22.0, screen.bottom() - size.y - 22.0),
            size,
        );
        let scale = if theme::reduced_motion() { 1.0 } else { 0.92 + 0.08 * vis };
        let c = rect.center().to_vec2();
        ctx.set_transform_layer(
            layer,
            egui::emath::TSTransform::from_translation(c)
                * egui::emath::TSTransform::from_scaling(scale)
                * egui::emath::TSTransform::from_translation(-c),
        );
        let mut painter = ctx.layer_painter(layer);
        painter.multiply_opacity(vis);
        painter.add(
            egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(130) }
                .as_shape(rect, CornerRadius::same(14)),
        );
        painter.rect_filled(rect, CornerRadius::same(14), Color32::from_rgba_unmultiplied(14, 14, 17, 246));
        painter.rect_stroke(rect, CornerRadius::same(14), Stroke::new(1.0, skin.line()), egui::StrokeKind::Inside);

        let p = &card.last;
        let done = card.done_at.is_some();
        let inner = rect.shrink2(Vec2::new(16.0, 14.0));
        let head = if done {
            "Удалено".to_string()
        } else if p.queued > 0 {
            format!("Удаление · ещё {}", p.queued)
        } else {
            "Удаление".to_string()
        };
        painter.text(inner.left_top(), Align2::LEFT_TOP, head, theme::display(13.0), skin.ink());
        let pct = if p.total_files > 0 { p.done_files as f32 / p.total_files as f32 } else { 0.0 };
        if !p.counting || done {
            painter.text(
                inner.right_top(),
                Align2::RIGHT_TOP,
                format!("{}%", (if done { 1.0 } else { pct } * 100.0).round() as u32),
                theme::mono(11.0),
                skin.dim(),
            );
        }
        crate::widgets::text_fit(
            &painter,
            inner.left_top() + Vec2::new(0.0, 21.0),
            Align2::LEFT_TOP,
            &p.title,
            theme::body(11.5),
            skin.dim(),
            inner.width(),
        );

        // Bar: eased fill, or a sweeping segment while files are counted.
        let bar = Rect::from_min_size(inner.left_top() + Vec2::new(0.0, 44.0), Vec2::new(inner.width(), 4.0));
        painter.rect_filled(bar, CornerRadius::same(2), skin.fill());
        if p.counting && !done {
            let t = ctx.input(|i| i.time) as f32;
            let seg = bar.width() * 0.3;
            let x = (t * 0.9).fract() * (bar.width() + seg) - seg;
            let sweep = Rect::from_min_max(
                Pos2::new(bar.left() + x.max(0.0), bar.top()),
                Pos2::new((bar.left() + x + seg).min(bar.right()), bar.bottom()),
            );
            if sweep.width() > 0.0 {
                painter.rect_filled(sweep, CornerRadius::same(2), skin.dim());
            }
        } else {
            let shown = theme::tween(ctx, Id::unique("remove-bar"), if done { 1.0 } else { pct }, 0.25);
            let fill = Rect::from_min_size(bar.min, Vec2::new((bar.width() * shown).max(2.0), 4.0));
            painter.rect_filled(fill, CornerRadius::same(2), skin.ink());
        }
        let freed = if done { card.freed } else { p.freed };
        let foot = if p.counting && !done {
            format!("считаю файлы… {}", p.total_files)
        } else {
            format!(
                "{} / {} файлов · освобождено {}",
                p.done_files,
                p.total_files,
                crate::util::human_bytes(freed)
            )
        };
        painter.text(inner.left_bottom(), Align2::LEFT_BOTTOM, foot, theme::mono(10.5), skin.faint());
    }

    fn start_update_download(&mut self, ctx: &egui::Context, info: crate::updater::UpdateInfo) {
        if self.update_task.as_ref().map(|t| !t.finished).unwrap_or(false) {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = cancel.clone();
        let ctx_task = ctx.clone();
        net::spawn(async move {
            let mut send = |ev: crate::updater::UpdateEvent| {
                let _ = tx.send(ev);
                ctx_task.request_repaint_after(Duration::from_millis(80));
            };
            if let Err(e) = crate::updater::download_installer(&net::client(), &info, &cancel_task, &mut send).await {
                send(crate::updater::UpdateEvent::Error(format!("{e:#}")));
            }
        });
        self.update_task = Some(UpdateTask {
            stage: "Подготовка…".into(),
            done: 0,
            total: 0,
            finished: false,
            ready_path: None,
            error: None,
            cancel,
            rx,
        });
    }

    /// Compact translucent update card when a newer release exists on GitHub.
    fn draw_update_banner(&mut self, ctx: &egui::Context, skin: &Skin) {
        if self.update_dismissed || !self.cfg.first_run_done {
            return;
        }
        let Some(info) = self.update_info.clone() else {
            return;
        };
        if self.cfg.skip_update_version == info.version {
            return;
        }
        let shown_at = self.update_shown_at.unwrap_or_else(Instant::now);
        let t = theme::arrive(ctx, shown_at, 0.0, 0.25);
        let screen = ctx.content_rect();
        let w = 368.0f32.min(screen.width() - 44.0);
        let bottom_offset = if self.remove_card.is_some() { 118.0 } else { 22.0 };
        let anchor = Pos2::new(screen.right() - w - 22.0, screen.bottom() - bottom_offset);

        let mut start_dl = false;
        let mut cancel_dl = false;
        let mut later = false;
        let mut never = false;

        egui::Area::new(Id::unique("update-banner"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(anchor)
            .show(ctx, |ui| {
                ui.multiply_opacity(t);
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(14, 14, 17, 246))
                    .stroke(Stroke::new(1.0, skin.line()))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(egui::Margin::symmetric(16, 14))
                    .shadow(egui::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(140),
                    })
                    .show(ui, |ui| {
                        ui.set_width(w - 32.0);
                        ui.horizontal(|ui| {
                            widgets::status_dot(ui, skin.green(), true);
                            ui.label(
                                RichText::new(format!("Доступно обновление v{}", info.version))
                                    .font(theme::display(14.0))
                                    .color(skin.ink()),
                            );
                            if info.size_bytes > 0 {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(crate::util::human_bytes(info.size_bytes))
                                            .font(theme::mono(10.0))
                                            .color(skin.green()),
                                    );
                                });
                            }
                        });
                        ui.add_space(4.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new("Установщик обновит только файл лаунчера — ник, сборки и миры сохранятся.")
                                    .font(theme::body(11.5))
                                    .color(skin.dim()),
                            )
                            .wrap(),
                        );

                        if let Some(task) = &self.update_task {
                            ui.add_space(8.0);
                            if let Some(err) = &task.error {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(err)
                                            .font(theme::body(11.5))
                                            .color(skin.red()),
                                    )
                                    .wrap(),
                                );
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    if widgets::button_sized(ui, skin, "Повторить", widgets::Btn::Primary, true, 28.0, 11.5, 11.0).clicked() {
                                        start_dl = true;
                                    }
                                    if widgets::button_sized(ui, skin, "Позже", widgets::Btn::Ghost, true, 28.0, 11.0, 10.0).clicked() {
                                        later = true;
                                    }
                                });
                            } else {
                                let pct = if task.total > 0 {
                                    format!("{}%", (task.frac() * 100.0).round() as u32)
                                } else {
                                    "…".to_string()
                                };
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&task.stage).font(theme::mono(10.0)).color(skin.dim()));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(RichText::new(pct).font(theme::mono(10.0)).color(skin.ink()));
                                    });
                                });
                                ui.add_space(4.0);
                                widgets::progress_bar(ui, skin, task.frac());
                                ui.add_space(6.0);
                                if widgets::button_sized(ui, skin, "Отменить", widgets::Btn::Ghost, true, 26.0, 11.0, 10.0).clicked() {
                                    cancel_dl = true;
                                }
                            }
                        } else {
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                if widgets::button_sized(ui, skin, "Обновиться", widgets::Btn::Primary, true, 28.0, 11.5, 12.0).clicked() {
                                    start_dl = true;
                                }
                                if widgets::button_sized(ui, skin, "Напомнить позже", widgets::Btn::Outline, true, 28.0, 11.0, 9.0).clicked() {
                                    later = true;
                                }
                                if widgets::button_sized(ui, skin, "Не показывать", widgets::Btn::Ghost, true, 28.0, 10.5, 8.0).clicked() {
                                    never = true;
                                }
                            });
                        }
                    });
            });

        if start_dl {
            self.update_task = None;
            self.start_update_download(ctx, info.clone());
        }
        if cancel_dl {
            if let Some(t) = &self.update_task {
                t.cancel.store(true, Ordering::SeqCst);
            }
        }
        if later {
            self.update_dismissed = true;
        }
        if never {
            self.cfg.skip_update_version = info.version;
            self.save_cfg();
            self.update_dismissed = true;
        }
    }

    /// «О лаунчере»: version, where files live (with «Открыть»), the active
    /// build, sources and the site link. Pops in from 94 % scale.
    fn draw_about(&mut self, ctx: &egui::Context, skin: &Skin) {
        let Some(at) = self.about_at else { return };
        let t = theme::arrive(ctx, at, 0.0, 0.22);
        let screen = ctx.content_rect();

        // Dimmed backdrop: a click outside or Esc closes the panel.
        let backdrop = egui::Area::new(Id::unique("about-backdrop"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(r, CornerRadius::ZERO, Color32::from_black_alpha((150.0 * t) as u8));
                resp
            })
            .inner;
        let mut close = backdrop.clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape));

        let size = Vec2::new(480.0, 470.0).min(screen.size() - Vec2::splat(40.0));
        let rect = Rect::from_center_size(screen.center(), size);
        let layer = egui::LayerId::new(egui::Order::Tooltip, Id::unique("about-card"));
        let scale = if theme::reduced_motion() { 1.0 } else { 0.94 + 0.06 * t };
        let c = rect.center().to_vec2();
        ctx.set_transform_layer(
            layer,
            egui::emath::TSTransform::from_translation(c)
                * egui::emath::TSTransform::from_scaling(scale)
                * egui::emath::TSTransform::from_translation(-c),
        );
        let data = config::data_dir();
        let game = self.cfg.game_dir();
        let logs = data.join("logs");
        let spec = self.active_spec();
        // The Java that will run this build, not merely the newest one.
        let java = java_label(
            &crate::java::fitting_cached(crate::java::required_for(&spec.mc), spec.loader).or_else(|| self.shared.java()),
        );
        egui::Area::new(Id::unique("about-card"))
            .order(egui::Order::Tooltip)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                ui.multiply_opacity(t);
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(14, 14, 17, 250))
                    .stroke(Stroke::new(1.0, skin.line()))
                    .corner_radius(CornerRadius::same(16))
                    .inner_margin(egui::Margin::same(22))
                    .shadow(egui::Shadow { offset: [0, 12], blur: 36, spread: 0, color: Color32::from_black_alpha(150) })
                    .show(ui, |ui| {
                        ui.set_width(size.x - 44.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(ui.available_width(), 28.0),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.label(egui::RichText::new("Sad Launcher").font(theme::display(20.0)).color(skin.ink()));
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new(format!("v{VERSION}")).font(theme::mono(10.5)).color(skin.faint()));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if widgets::button_sized(ui, skin, "Закрыть", widgets::Btn::Ghost, true, 26.0, 11.5, 10.0).clicked() {
                                        close = true;
                                    }
                                });
                            },
                        );
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!(
                                    "Лаунчер Minecraft на Rust. Бесплатный, без регистрации и рекламы. Сайт: {}",
                                    config::SITE_URL
                                ))
                                .font(theme::body(12.0))
                                .color(skin.dim()),
                            )
                            .wrap(),
                        );
                        ui.add_space(14.0);

                        let mut open: Option<std::path::PathBuf> = None;
                        let mut path_row = |ui: &mut Ui, key: &str, path: &std::path::Path| {
                            let row_w = ui.available_width();
                            ui.allocate_ui_with_layout(
                                Vec2::new(row_w, 26.0),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(66.0, 26.0),
                                        Layout::left_to_right(Align::Center),
                                        |ui| {
                                            ui.label(egui::RichText::new(key).font(theme::body(11.5)).color(skin.faint()));
                                        },
                                    );
                                    let mid_w = (row_w - 66.0 - 82.0).max(80.0);
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(mid_w, 26.0),
                                        Layout::left_to_right(Align::Center),
                                        |ui| {
                                            ui.set_max_width(mid_w);
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(path.display().to_string())
                                                        .font(theme::mono(10.0))
                                                        .color(skin.dim()),
                                                )
                                                .truncate(),
                                            );
                                        },
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if widgets::button_sized(ui, skin, "Открыть", widgets::Btn::Outline, true, 22.0, 10.5, 9.0).clicked() {
                                            open = Some(path.to_path_buf());
                                        }
                                    });
                                },
                            );
                        };
                        path_row(ui, "Данные", &data);
                        path_row(ui, "Игра", &game);
                        path_row(ui, "Журналы", &logs);
                        if let Some(p) = open {
                            crate::util::open_path(&p);
                        }
                        ui.add_space(8.0);
                        widgets::kv_row(ui, skin, "Сборка", &spec.title(), false);
                        widgets::kv_row(ui, skin, "Java", &java, false);
                        widgets::kv_row(ui, skin, "Сайт", config::SITE_URL, true);
                        widgets::kv_row(ui, skin, "Каталоги", "Modrinth, CurseForge", false);
                        widgets::kv_row(ui, skin, "Загрузчики", "Forge, NeoForge, Fabric, Quilt", false);

                        ui.add_space(14.0);
                        for tip in [
                            "Игра вылетела: нажми «Играть» ещё раз. Лаунчер отключит несовместимые моды и докачает зависимости.",
                            "Выключенные моды остаются в папке сборки с расширением .disabled.",
                            "Порт сервера, память и обои: вкладка «Настройки».",
                        ] {
                            ui.add(egui::Label::new(egui::RichText::new(tip).font(theme::body(11.5)).color(skin.dim())).wrap());
                            ui.add_space(4.0);
                        }
                        ui.add_space(12.0);
                        self.update_row(ui, skin);
                        ui.add_space(10.0);
                        let mut copy_site = false;
                        ui.horizontal(|ui| {
                            if widgets::button_icon(
                                ui,
                                skin,
                                crate::icons::Icon::Globe,
                                "Открыть сайт лаунчера",
                                widgets::Btn::Primary,
                                true,
                                30.0,
                                12.0,
                                13.0,
                            )
                            .on_hover_text(config::SITE_URL)
                            .clicked()
                            {
                                let _ = open::that_detached(config::SITE_URL);
                            }
                            if widgets::button_icon(
                                ui,
                                skin,
                                crate::icons::Icon::Copy,
                                "Скопировать ссылку",
                                widgets::Btn::Outline,
                                true,
                                30.0,
                                11.5,
                                11.0,
                            )
                            .clicked()
                            {
                                copy_site = true;
                            }
                        });
                        if copy_site {
                            crate::util::copy_text(ctx, config::SITE_URL);
                        }
                    });
            });
        if close {
            self.about_at = None;
        }
    }

    fn record_nav(&mut self) {
        if self.nav_lock {
            self.nav_lock = false;
            return;
        }
        let cur = NavEntry {
            tab: self.tab,
            mods_page: self.mods_page,
            detail: self.detail.clone(),
        };
        if self.nav_history.get(self.nav_pos).map(|e| e.same(&cur)).unwrap_or(false) {
            return;
        }
        if self.nav_pos + 1 < self.nav_history.len() {
            self.nav_history.truncate(self.nav_pos + 1);
        }
        self.nav_history.push(cur);
        if self.nav_history.len() > 40 {
            self.nav_history.remove(0);
        }
        self.nav_pos = self.nav_history.len().saturating_sub(1);
    }

    fn apply_nav(&mut self, entry: NavEntry) {
        self.nav_lock = true;
        if self.tab != entry.tab {
            self.tab = entry.tab;
            self.tab_at = Instant::now();
        }
        self.mods_page = entry.mods_page;
        let same_detail = self.detail.as_ref().map(|d| (&d.source, &d.id))
            == entry.detail.as_ref().map(|d| (&d.source, &d.id));
        if !same_detail {
            match entry.detail {
                Some(hit) => self.open_detail(hit),
                None => {
                    self.detail = None;
                    self.detail_rx = None;
                    self.detail_body = None;
                }
            }
        }
    }

    pub fn nav_back(&mut self) {
        if self.about_at.is_some() {
            self.about_at = None;
            return;
        }
        if self.nav_pos > 0 {
            self.nav_pos -= 1;
            let target = self.nav_history[self.nav_pos].clone();
            self.apply_nav(target);
        }
    }

    pub fn nav_forward(&mut self) {
        if self.nav_pos + 1 < self.nav_history.len() {
            self.nav_pos += 1;
            let target = self.nav_history[self.nav_pos].clone();
            self.apply_nav(target);
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.bind_capture.is_some() {
            return;
        }
        // The viewer owns Esc and the arrows while it is open; the mouse
        // «back» button closes it.
        if self.viewer.is_some() {
            if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Extra1)) {
                self.viewer = None;
                self.video_state = None;
            }
            return;
        }
        let (back_mouse, fwd_mouse, alt_left, alt_right, esc) = ctx.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Extra1),
                i.pointer.button_pressed(egui::PointerButton::Extra2),
                i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft),
                i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if back_mouse || alt_left {
            self.nav_back();
            return;
        }
        if fwd_mouse || alt_right {
            self.nav_forward();
            return;
        }
        if esc {
            if self.middle_scroll_anchor.take().is_some() {
                return;
            }
            if self.about_at.take().is_some() {
                return;
            }
            self.confirm_delete = None;
            self.confirm_uninstall = false;
            self.confirm_reset = false;
            self.confirm_wipe = false;
            self.build_confirm_del = None;
        }

        let typing = ctx.egui_wants_keyboard_input();
        let (tab_idx, cycle, find, refresh, scroll_y) = ctx.input(|i| {
            let mut t = None;
            let mut cyc = 0i32;
            let mut f = false;
            let mut r = false;
            let mut sy = 0.0f32;
            if i.modifiers.ctrl {
                for (k, idx) in [
                    (egui::Key::Num1, 0usize),
                    (egui::Key::Num2, 1),
                    (egui::Key::Num3, 2),
                    (egui::Key::Num4, 3),
                    (egui::Key::Num5, 4),
                    (egui::Key::Num6, 5),
                    (egui::Key::Num7, 6),
                ] {
                    if i.key_pressed(k) {
                        t = Some(idx);
                    }
                }
                if i.key_pressed(egui::Key::Tab) {
                    cyc = if i.modifiers.shift { -1 } else { 1 };
                }
                if i.key_pressed(egui::Key::F) || i.key_pressed(egui::Key::K) {
                    f = true;
                }
                if i.key_pressed(egui::Key::R) {
                    r = true;
                }
            }
            if i.key_pressed(egui::Key::F5) {
                r = true;
            }
            if !typing {
                if i.key_pressed(egui::Key::PageDown) {
                    sy -= 420.0;
                }
                if i.key_pressed(egui::Key::PageUp) {
                    sy += 420.0;
                }
                if i.key_pressed(egui::Key::Home) {
                    sy += 4000.0;
                }
                if i.key_pressed(egui::Key::End) {
                    sy -= 4000.0;
                }
            }
            (t, cyc, f, r, sy)
        });

        let tabs = Tab::all();
        if let Some(idx) = tab_idx {
            if let Some(&tab) = tabs.get(idx) {
                self.go(tab);
            }
        }
        if cycle != 0 {
            if let Some(cur) = tabs.iter().position(|t| *t == self.tab) {
                let next = (cur as i32 + cycle).rem_euclid(tabs.len() as i32) as usize;
                self.go(tabs[next]);
            }
        }
        if find {
            if !matches!(self.tab, Tab::Mods | Tab::Versions | Tab::Build) {
                self.go(Tab::Mods);
                self.mods_page = 0;
            } else if self.tab == Tab::Mods {
                self.mods_page = 0;
                self.detail = None;
            }
            self.focus_search = true;
        }
        if refresh {
            match self.tab {
                Tab::Versions => minecraft::spawn_manifest(self.shared.clone()),
                Tab::Mods => {
                    if self.mods_page == 0 {
                        self.search.reset();
                        self.start_search(ctx);
                    } else {
                        self.touch_mods();
                    }
                }
                _ => {
                    self.refresh_installed();
                    self.touch_mods();
                }
            }
            self.toast("Обновлено");
        }
        if scroll_y.abs() > 0.1 {
            ctx.input_mut(|i| i.smooth_scroll_delta.y += scroll_y);
        }
    }

    /// Injects middle-mouse auto-scroll delta BEFORE any ScrollArea renders this frame.
    fn pump_middle_scroll(&mut self, ctx: &egui::Context) {
        let (mid_pressed, mid_down, mid_released, other_click, pointer_pos, dt) = ctx.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Middle),
                i.pointer.button_down(egui::PointerButton::Middle),
                i.pointer.button_released(egui::PointerButton::Middle),
                i.pointer.button_pressed(egui::PointerButton::Primary)
                    || i.pointer.button_pressed(egui::PointerButton::Secondary),
                i.pointer.latest_pos().or_else(|| i.pointer.hover_pos()),
                i.stable_dt.clamp(0.004, 0.05),
            )
        });

        if mid_pressed {
            if self.middle_scroll_anchor.is_some() {
                self.middle_scroll_anchor = None;
                self.middle_scroll_dragged = false;
                return;
            } else if let Some(pos) = pointer_pos {
                self.middle_scroll_anchor = Some(pos);
                self.middle_scroll_dragged = false;
            }
        }

        if let Some(anchor) = self.middle_scroll_anchor {
            if other_click || (mid_released && self.middle_scroll_dragged) {
                self.middle_scroll_anchor = None;
                self.middle_scroll_dragged = false;
                return;
            }
            if let Some(pos) = pointer_pos {
                let dy = pos.y - anchor.y;
                if mid_down && (pos - anchor).length() > 8.0 {
                    self.middle_scroll_dragged = true;
                }
                if dy.abs() > 4.0 {
                    let mag = (dy.abs() - 4.0).powf(1.22) * 0.45 * (dt * 60.0);
                    let delta = -dy.signum() * mag;
                    ctx.input_mut(|i| {
                        i.smooth_scroll_delta.y += delta;
                    });
                }
            }
            ctx.set_cursor_icon(CursorIcon::ResizeVertical);
            ctx.request_repaint();
        }
    }

    fn draw_middle_scroll_anchor(&self, ctx: &egui::Context, skin: &Skin) {
        if let Some(anchor) = self.middle_scroll_anchor {
            let p = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                Id::unique("mid-scroll-anchor"),
            ));
            p.circle_filled(anchor, 13.0, Color32::from_rgba_unmultiplied(14, 14, 17, 230));
            p.circle_stroke(anchor, 13.0, Stroke::new(1.0, skin.ink().gamma_multiply(0.75)));
            p.circle_filled(anchor, 2.3, skin.ink());
            p.line_segment(
                [anchor + Vec2::new(-3.0, -5.5), anchor + Vec2::new(0.0, -8.5)],
                Stroke::new(1.3, skin.dim()),
            );
            p.line_segment(
                [anchor + Vec2::new(0.0, -8.5), anchor + Vec2::new(3.0, -5.5)],
                Stroke::new(1.3, skin.dim()),
            );
            p.line_segment(
                [anchor + Vec2::new(-3.0, 5.5), anchor + Vec2::new(0.0, 8.5)],
                Stroke::new(1.3, skin.dim()),
            );
            p.line_segment(
                [anchor + Vec2::new(0.0, 8.5), anchor + Vec2::new(3.0, 5.5)],
                Stroke::new(1.3, skin.dim()),
            );
        }
    }

    fn draw_toasts(&self, ctx: &egui::Context, skin: &Skin) {
        if self.toasts.is_empty() {
            return;
        }
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            Id::unique("toasts"),
        ));
        let screen = ctx.content_rect();
        let mut y = screen.bottom() - 28.0;
        for t in self.toasts.iter().rev() {
            let age = t.at.elapsed().as_secs_f32();
            let enter = theme::ease_out(age / 0.28);
            let exit = ((5.0 - age) / 0.45).clamp(0.0, 1.0);
            let mut painter = painter.clone();
            painter.multiply_opacity(enter * exit);
            let galley = painter.layout_no_wrap(t.text.clone(), theme::body(12.5), skin.ink());
            let w = (galley.size().x + 28.0).min(screen.width() - 80.0);
            let rise = theme::travel(14.0) * (1.0 - enter);
            let rect = Rect::from_min_size(
                Pos2::new(screen.center().x - w / 2.0, y - 32.0 + rise),
                Vec2::new(w, 32.0),
            );
            painter.add(
                egui::Shadow {
                    offset: [0, 6],
                    blur: 20,
                    spread: 0,
                    color: Color32::from_black_alpha(120),
                }
                .as_shape(rect, CornerRadius::same(16)),
            );
            painter.rect_filled(
                rect,
                CornerRadius::same(16),
                Color32::from_rgba_unmultiplied(16, 16, 19, 244),
            );
            painter.rect_stroke(
                rect,
                CornerRadius::same(16),
                Stroke::new(1.0, skin.line()),
                egui::StrokeKind::Inside,
            );
            painter.galley(
                Pos2::new(rect.left() + 14.0, rect.center().y - galley.size().y / 2.0),
                galley,
                skin.ink(),
            );
            y -= 40.0 * enter.max(0.3);
        }
    }
}

impl eframe::App for Launcher {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.effects_dirty {
            crate::effects::save(&self.effects);
        }
        if self.cfg_dirty && crate::util::nickname_valid(&self.cfg.nickname) {
            self.save_cfg();
        }
        self.shutdown_online();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let bench_at = bench::frame_start(&ctx);
        self.ui_frame(ui, &ctx);
        self.sync_video(&ctx, frame);
        bench::frame_end(&ctx, bench_at);
        bench::screenshot(&ctx);
    }
}

/// Developer benchmark: `MADRID_BENCH=<seconds>` repaints every frame for
/// that long, then writes «frames ui_ms_avg» to `MADRID_BENCH_OUT` and
/// closes. Lets per-frame cost be compared before/after a change.
mod bench {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};

    static FRAMES: AtomicU64 = AtomicU64::new(0);
    static UI_NANOS: AtomicU64 = AtomicU64::new(0);

    fn config() -> Option<&'static (Duration, Instant)> {
        static CFG: OnceLock<Option<(Duration, Instant)>> = OnceLock::new();
        CFG.get_or_init(|| {
            let secs: f32 = std::env::var("MADRID_BENCH").ok()?.parse().ok()?;
            Some((Duration::from_secs_f32(secs.clamp(1.0, 120.0)), Instant::now()))
        })
        .as_ref()
    }

    pub fn frame_start(ctx: &egui::Context) -> Option<Instant> {
        config()?;
        ctx.request_repaint();
        Some(Instant::now())
    }

    /// `MADRID_DEV_SHOT=<file.png>`: after `MADRID_DEV_SHOT_DELAY` seconds
    /// (default 8) saves the window as rendered (alpha kept) and closes.
    /// With `MADRID_DEV_ZOOM=2` that is a crisp 2× shot for the website.
    pub fn screenshot(ctx: &egui::Context) {
        static STARTED: OnceLock<Option<(String, Instant, f32)>> = OnceLock::new();
        static ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        let Some((path, started, delay)) = STARTED
            .get_or_init(|| {
                let path = std::env::var("MADRID_DEV_SHOT").ok()?;
                let delay = std::env::var("MADRID_DEV_SHOT_DELAY").ok().and_then(|d| d.parse().ok()).unwrap_or(8.0);
                Some((path, Instant::now(), delay))
            })
            .as_ref()
        else {
            return;
        };
        // Continuous frames until the shot is taken (an idle, unfocused
        // window may otherwise not paint the frame the request rides on).
        ctx.request_repaint();
        if started.elapsed().as_secs_f32() >= *delay && !ASKED.swap(true, Ordering::SeqCst) {
            let path = path.clone();
            let ctx2 = ctx.clone();
            ctx.request_screenshot(move |img| {
                let [w, h] = img.size;
                let raw: Vec<u8> = img.pixels.iter().flat_map(|p| p.to_srgba_unmultiplied()).collect();
                if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, raw) {
                    if let Err(e) = buf.save(&path) {
                        let _ = std::fs::write(format!("{path}.err"), e.to_string());
                    }
                }
                ctx2.send_viewport_cmd(egui::ViewportCommand::Close);
                ctx2.request_repaint();
            });
        }
    }

    pub fn frame_end(ctx: &egui::Context, at: Option<Instant>) {
        let (Some(at), Some((span, started))) = (at, config()) else {
            return;
        };
        // The first 3 s settle start-up work (fonts, caches) out of the number.
        if started.elapsed() < Duration::from_secs(3) {
            return;
        }
        FRAMES.fetch_add(1, Ordering::Relaxed);
        UI_NANOS.fetch_add(at.elapsed().as_nanos() as u64, Ordering::Relaxed);
        if started.elapsed() >= *span + Duration::from_secs(3) {
            let frames = FRAMES.load(Ordering::Relaxed).max(1);
            let ui_ms = UI_NANOS.load(Ordering::Relaxed) as f64 / frames as f64 / 1e6;
            if let Ok(out) = std::env::var("MADRID_BENCH_OUT") {
                let _ = std::fs::write(out, format!("{frames} {ui_ms:.3}"));
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl Launcher {
    fn ui_frame(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let ctx = ctx.clone();
        self.poll(&ctx);
        self.handle_shortcuts(&ctx);
        self.pump_middle_scroll(&ctx);
        self.ensure_wallpaper(&ctx);

        if (self.cfg.transparency - self.last_alpha).abs() > 0.001 {
            self.last_alpha = self.cfg.transparency;
            theme::style(&ctx, &Skin::new(self.cfg.transparency));
        }
        let skin = Skin::new(self.cfg.transparency);

        let full = ui.max_rect();
        if let Some(tex) = &self.wallpaper_tex {
            ui.painter().image(
                tex.id(),
                full,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        ui.painter()
            .rect_filled(full, CornerRadius::same(18), skin.base());
        ui.painter().rect_stroke(
            full,
            CornerRadius::same(18),
            Stroke::new(1.0, skin.line_soft()),
            egui::StrokeKind::Inside,
        );

        let inner = full.shrink(1.0);
        let mut root = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner)
                .layout(Layout::top_down(Align::Min)),
        );
        root.spacing_mut().item_spacing = Vec2::ZERO;
        root.set_width(inner.width());

        let bar_rect = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 46.0));
        let mut bar_ui = root.new_child(
            egui::UiBuilder::new()
                .max_rect(bar_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        self.titlebar(&mut bar_ui, &skin);
        root.painter().line_segment(
            [
                Pos2::new(inner.left() + 1.0, bar_rect.bottom()),
                Pos2::new(inner.right() - 1.0, bar_rect.bottom()),
            ],
            Stroke::new(1.0, skin.line_soft()),
        );

        let body = Rect::from_min_max(
            Pos2::new(inner.left(), bar_rect.bottom() + 1.0),
            inner.max,
        );
        let mut body_ui = root.new_child(
            egui::UiBuilder::new()
                .max_rect(body)
                .layout(Layout::left_to_right(Align::Min)),
        );
        body_ui.spacing_mut().item_spacing = Vec2::ZERO;

        let rail_w = 228.0;
        let mut rail_ui = body_ui.new_child(
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_size(body.min, Vec2::new(rail_w, body.height())))
                .layout(Layout::top_down(Align::Min)),
        );
        self.rail(&mut rail_ui, &skin);

        let content_rect = Rect::from_min_max(
            Pos2::new(body.left() + rail_w, body.top()),
            body.max,
        );
        let mut content_ui = body_ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        content_ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 26,
                right: 26,
                top: 20,
                bottom: 18,
            })
            .show(&mut content_ui, |ui| {
                ui.set_width(ui.available_width());
                // Page change: content settles in from 10 px below while it
                // brightens — starts already visible, never a blank frame.
                let appear = theme::arrive(&ctx, self.tab_at, 0.0, 0.24);
                ui.multiply_opacity(0.35 + 0.65 * appear);
                let rise = theme::travel(10.0) * (1.0 - appear);
                if rise > 0.05 {
                    ui.add_space(rise);
                }
                match self.tab {
                    Tab::Play => self.view_play(ui, &skin),
                    Tab::Versions => self.view_versions(ui, &skin),
                    Tab::Build => self.view_build(ui, &skin),
                    Tab::Mods => self.view_mods(ui, &skin),
                    Tab::Effects => self.view_effects(ui, &skin),
                    Tab::Server => self.view_server(ui, &skin),
                    Tab::Settings => self.view_settings(ui, &skin),
                }
            });

        self.draw_remove_card(&ctx, &skin);
        self.draw_update_banner(&ctx, &skin);
        self.draw_about(&ctx, &skin);
        self.draw_viewer(&ctx, &skin);
        self.draw_middle_scroll_anchor(&ctx, &skin);
        self.draw_toasts(&ctx, &skin);
        self.resize_edges(ui);
        self.nickname_screen(ui, &skin);
        self.record_nav();
    }
}

pub fn new_shared(cfg: Config) -> Arc<Shared> {
    config::ensure_dirs();
    let shared = Arc::new(Shared::new(cfg.clone()));
    shared.log(
        LogKind::Info,
        "Sad Launcher запущен — сервер поднимается вместе с окном",
    );
    shared
}

pub fn java_label(java: &Option<JavaInfo>) -> String {
    match java {
        Some(j) => format!("Java {} · {}-bit", j.version, j.bits),
        None => "Java не найдена".into(),
    }
}
