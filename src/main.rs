#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod automation;
mod categories;
mod deps;
mod effects;
mod ftb;
mod packinfo;
mod smart;
mod config;
mod icons;
mod java;
mod launch;
mod loader;
mod minecraft;
mod net;
mod mods;
pub mod profiles;
mod remove;
mod platform;
mod pomme;
mod room;
mod server;
mod shared;
mod theme;
mod unpack;
mod updater;
mod upnp;
mod util;
mod views;
mod widgets;

fn main() {
    // MADRID_SECOND_INSTANCE=1 lets a developer run a build next to the
    // installed one (memory/perf measurements, screenshots); users never set
    // it. Such a copy never touches shortcuts, autostart or folder names.
    let dev_copy = std::env::var_os("MADRID_SECOND_INSTANCE").is_some();
    // «LLM Launcher Madrid» → «Sad Launcher»: move the old data folder once.
    if !dev_copy {
        config::migrate_legacy();
    }
    config::ensure_dirs();
    let mut cfg = config::load();

    // One window per machine: a second copy just brings the first forward.
    let mut _instance = util::lock_single_instance();
    // Started by the self-updater while the old copy is still closing
    // (stopping its server): wait for it instead of quitting.
    if _instance.is_none() && std::env::var_os("MADRID_RELAUNCH_WAIT").is_some() {
        for _ in 0..120 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            _instance = util::lock_single_instance();
            if _instance.is_some() {
                break;
            }
        }
    }
    if _instance.is_none() && !dev_copy {
        util::focus_window(config::APP_TITLE);
        return;
    }

    // Older versions kept one shared mods folder; give each build its own.
    profiles::migrate_to_instances(&mut cfg);
    if !dev_copy {
        std::thread::spawn(util::migrate_legacy_names);
    }

    // First run: put the launcher on the Desktop with its own icon.
    if !dev_copy && (!cfg.shortcut_done || !util::shortcut_exists()) {
        if util::create_desktop_shortcut() {
            cfg.shortcut_done = true;
            config::save(&cfg);
        }
    }

    let shared = app::new_shared(cfg.clone());

    // The Madrid server boots with the window: the launcher is the host.
    server::bootstrap(shared.clone());

    let title = config::APP_TITLE;
    std::thread::spawn(move || util::dress_window(config::APP_TITLE));
    std::thread::spawn(|| util::prune_data(&config::data_dir()));
    // Folders left in the trash by a delete cut short.
    remove::sweep_leftovers();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size({
                // MADRID_DEV_ZOOM (website screenshots): same layout, more pixels.
                let z = std::env::var("MADRID_DEV_ZOOM").ok().and_then(|z| z.parse::<f32>().ok()).unwrap_or(1.0).clamp(0.5, 3.0);
                [1240.0 * z, 760.0 * z]
            })
            .with_min_inner_size([960.0, 620.0])
            .with_transparent(true)
            .with_decorations(false)
            .with_resizable(true)
            .with_icon(window_icon()),
        ..Default::default()
    };

    let shared_for_app = shared.clone();
    let cfg_for_app = cfg.clone();
    let result = eframe::run_native(
        title,
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::Launcher::new(
                cc,
                cfg_for_app,
                shared_for_app,
            )))
        }),
    );

    // Window closed — the hosted server goes down with it.
    server::shutdown(&shared);

    if let Err(e) = result {
        let msg = format!("Sad Launcher не смог открыть окно: {e}");
        eprintln!("{msg}");
        util::message_box(&msg);
    }
}

/// Taskbar / Alt-Tab icon: the same white «S» tile as the exe.
fn window_icon() -> std::sync::Arc<egui::IconData> {
    let img = image::load_from_memory(include_bytes!("../assets/icon.png"))
        .map(|i| i.into_rgba8())
        .ok();
    std::sync::Arc::new(match img {
        Some(i) => egui::IconData { width: i.width(), height: i.height(), rgba: i.into_raw() },
        None => egui::IconData::default(),
    })
}
