import io

p = "src/app.rs"
src = io.open(p, encoding="utf-8").read()
count_before = src

src = src.replace("use std::sync::Arc;\nuse std::time::{Duration, Instant};",
"use std::sync::atomic::{AtomicBool, Ordering};\nuse std::sync::Arc;\nuse std::time::{Duration, Instant};")

# InstallTask + cancel
src = src.replace('''pub struct InstallTask {
    pub version: String,
    pub loader: String,
    pub stage: String,
    pub done: u64,
    pub total: u64,
    pub finished: bool,
    pub ok: bool,
    pub error: Option<String>,
    pub rx: Receiver<InstallEvent>,
}''', '''pub struct InstallTask {
    pub version: String,
    pub loader: String,
    pub stage: String,
    pub done: u64,
    pub total: u64,
    pub finished: bool,
    pub ok: bool,
    pub error: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub rx: Receiver<InstallEvent>,
}''')

# ModTask cancel
src = src.replace('''pub struct ModTask {
    pub title: String,
    pub done: u64,
    pub total: u64,
    pub finished: bool,
    pub error: Option<String>,
    pub path: Option<PathBuf>,
    pub rx: Receiver<ModEvent>,
}''', '''pub struct ModTask {
    pub title: String,
    pub done: u64,
    pub total: u64,
    pub finished: bool,
    pub error: Option<String>,
    pub path: Option<PathBuf>,
    pub cancel: Arc<AtomicBool>,
    pub rx: Receiver<ModEvent>,
}''')

# BuildTask cancel
src = src.replace('''pub struct BuildTask {
    pub total: usize,
    pub done: usize,
    pub finished: bool,
    pub error: Option<String>,
    pub rx: Receiver<Result<PathBuf, String>>,
}''', '''pub struct BuildTask {
    pub total: usize,
    pub done: usize,
    pub finished: bool,
    pub error: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub rx: Receiver<Result<PathBuf, String>>,
}''')

# JavaTask cancel
src = src.replace('''pub struct JavaTask {
    pub major: u32,
    pub done: u64,
    pub total: u64,
    pub finished: bool,
    pub error: Option<String>,
    pub rx: Receiver<JavaEvent>,
}''', '''pub struct JavaTask {
    pub major: u32,
    pub done: u64,
    pub total: u64,
    pub finished: bool,
    pub error: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub rx: Receiver<JavaEvent>,
}''')

# start_install
src = src.replace('''        let (tx, rx) = std::sync::mpsc::channel::<InstallEvent>();
        let cfg = self.cfg.clone();
        let ctx = ctx.clone();
        let mc_task = mc.clone();
        let loader_task = loader.clone();''', '''        let (tx, rx) = std::sync::mpsc::channel::<InstallEvent>();
        let cfg = self.cfg.clone();
        let ctx = ctx.clone();
        let mc_task = mc.clone();
        let loader_task = loader.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = cancel.clone();''')

src = src.replace('''                    if let Err(e) =
                        minecraft::install(&client, &cfg, &mc_task, &loader_task, &mut send).await
                    {
                        let _ = send(InstallEvent::Error(format!("{e:#}")));
                    }''', '''                    if let Err(e) =
                        minecraft::install(&client, &cfg, &mc_task, &loader_task, &cancel_task, &mut send).await
                    {
                        let _ = send(InstallEvent::Error(format!("{e:#}")));
                    }''')

src = src.replace('''        self.install = Some(InstallTask {
            version: mc,
            loader,
            stage: "Подготовка…".into(),
            done: 0,
            total: 0,
            finished: false,
            ok: false,
            error: None,
            rx,
        });''', '''        self.install = Some(InstallTask {
            version: mc,
            loader,
            stage: "Подготовка…".into(),
            done: 0,
            total: 0,
            finished: false,
            ok: false,
            error: None,
            cancel,
            rx,
        });''')

# start_mod_install
src = src.replace('''        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        let title = hit.title.clone();
        let kind = hit.kind.clone();''', '''        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        let title = hit.title.clone();
        let kind = hit.kind.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = cancel.clone();''')

src = src.replace('''                    if let Err(e) =
                        mods::install(&client, &cfg, &hit, &mc, &loader, &kind, &mut send).await
                    {
                        let _ = send(ModEvent::Error(format!("{e:#}")));
                    }''', '''                    if let Err(e) =
                        mods::install(&client, &cfg, &hit, &mc, &loader, &kind, &cancel_task, &mut send).await
                    {
                        let _ = send(ModEvent::Error(format!("{e:#}")));
                    }''')

src = src.replace('''        self.mod_task = Some(ModTask {
            title,
            done: 0,
            total: 0,
            finished: false,
            error: None,
            path: None,
            rx,
        });''', '''        self.mod_task = Some(ModTask {
            title,
            done: 0,
            total: 0,
            finished: false,
            error: None,
            path: None,
            cancel,
            rx,
        });''')

# start_build_install
src = src.replace('''        let entries = self.build.clone();
        let cfg = self.cfg.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Result<PathBuf, String>>();
        let ctx = ctx.clone();
        let total = entries.len();''', '''        let entries = self.build.clone();
        let cfg = self.cfg.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Result<PathBuf, String>>();
        let ctx = ctx.clone();
        let total = entries.len();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = cancel.clone();''')

src = src.replace('''                    for entry in &entries {
                        let hit = ModHit {''', '''                    for entry in &entries {
                        if cancel_task.load(Ordering::Relaxed) {
                            break;
                        }
                        let hit = ModHit {''')

src = src.replace('''                        let res = mods::install(&client, &cfg, &hit, mc_eff, loader_eff, &entry.kind, &mut noop)
                            .await
                            .map_err(|e| format!("{}: {e:#}", entry.title));''', '''                        let res = mods::install(&client, &cfg, &hit, mc_eff, loader_eff, &entry.kind, &cancel_task, &mut noop)
                            .await
                            .map_err(|e| format!("{}: {e:#}", entry.title));''')

src = src.replace('''        self.build_task = Some(BuildTask {
            total,
            done: 0,
            finished: false,
            error: None,
            rx,
        });''', '''        self.build_task = Some(BuildTask {
            total,
            done: 0,
            finished: false,
            error: None,
            cancel,
            rx,
        });''')

# start_java_download
src = src.replace('''        let (tx, rx) = std::sync::mpsc::channel::<JavaEvent>();
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("java-install".into())''', '''        let (tx, rx) = std::sync::mpsc::channel::<JavaEvent>();
        let ctx = ctx.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = cancel.clone();
        std::thread::Builder::new()
            .name("java-install".into())''')

src = src.replace('''                    if let Err(e) = java::download_runtime(&client, major, &mut send).await {
                        let _ = send(JavaEvent::Error(format!("{e:#}")));
                    }''', '''                    if let Err(e) = java::download_runtime(&client, major, &cancel_task, &mut send).await {
                        let _ = send(JavaEvent::Error(format!("{e:#}")));
                    }''')

src = src.replace('''        self.java_task = Some(JavaTask {
            major,
            done: 0,
            total: 0,
            finished: false,
            error: None,
            rx,
        });''', '''        self.java_task = Some(JavaTask {
            major,
            done: 0,
            total: 0,
            finished: false,
            error: None,
            cancel,
            rx,
        });''')

# poll: neutral toasts for Отменено
src = src.replace('''                } else {
                    self.toast(format!(
                        "Не установилось: {}",
                        task.error.unwrap_or_default()
                    ));
                }''', '''                } else if task.error.as_deref().map(|e| e.contains("Отменено")).unwrap_or(false) {
                    self.toast("Установка отменена");
                } else {
                    self.toast(format!(
                        "Не установилось: {}",
                        task.error.unwrap_or_default()
                    ));
                }''')

src = src.replace('''                } else {
                    self.toast(format!("Мод не скачался: {}", task.error.unwrap_or_default()));
                }''', '''                } else if task.error.as_deref().map(|e| e.contains("Отменено")).unwrap_or(false) {
                    self.toast("Скачивание отменено");
                } else {
                    self.toast(format!("Мод не скачался: {}", task.error.unwrap_or_default()));
                }''')

assert src != count_before
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("app.rs cancel ok")
