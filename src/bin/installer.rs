//! Modern GUI Installer for Sad Launcher.
//! Beautiful single-window wizard matching the launcher's sleek aesthetic.
//! Installs sad-launcher.exe and shortcuts without weird errors.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// The installer is a Windows wizard (Programs folder, .lnk shortcuts) with
// the launcher exe baked in. On Linux/macOS the launcher is built from
// source and needs no installer, so this binary only explains that.
#[cfg(not(windows))]
fn main() {
    eprintln!("Установщик нужен только для Windows. На Linux и macOS соберите лаунчер: cargo build --release --bin sad-launcher");
}

#[cfg(windows)]
fn main() {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() == Some(std::ffi::OsStr::new("--update")) {
        if let Some(target) = args.next() {
            app::run_silent_update(std::path::PathBuf::from(target));
            return;
        }
    }
    app::main();
}

#[cfg(windows)]
#[path = "../win.rs"]
mod win;

#[cfg(windows)]
mod app {

use eframe::egui::{
    self, Align, Color32, CornerRadius, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2,
};
use rfd::FileDialog;
use std::path::PathBuf;

/// The launcher exe, baked in at build time by scripts/make-dist.sh.
static PAYLOAD: &[u8] = include_bytes!("../../assets/launcher-payload.exe");

fn default_install_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Programs")
        .join("Sad Launcher")
}

fn desktop_dir() -> PathBuf {
    dirs::desktop_dir().unwrap_or_else(|| {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Desktop")
    })
}

fn programs_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
}

/// Native .lnk through the shell's COM object (was a PowerShell script).
fn make_shortcut(exe_path: &PathBuf, link_path: &PathBuf) -> bool {
    crate::win::create_shortcut(link_path, exe_path, "Sad Launcher · Minecraft Forge")
}

#[derive(Clone, PartialEq, Eq)]
enum Step {
    Config,
    Installing,
    Done,
    Error(String),
}

struct InstallerApp {
    install_dir: PathBuf,
    shortcut_desktop: bool,
    shortcut_start: bool,
    launch_after: bool,
    step: Step,
    progress: f32,
    progress_status: String,
    process_running: bool,
}

impl Default for InstallerApp {
    fn default() -> Self {
        let dir = default_install_dir();
        let running = is_launcher_running(&dir);
        Self {
            install_dir: dir,
            shortcut_desktop: true,
            shortcut_start: true,
            launch_after: true,
            step: Step::Config,
            progress: 0.0,
            progress_status: String::new(),
            process_running: running,
        }
    }
}

fn is_launcher_running(dir: &PathBuf) -> bool {
    let target = dir.join("sad-launcher.exe");
    if !target.is_file() {
        return false;
    }
    // Attempt to open the file with write permissions to detect if it's locked by a running process
    match std::fs::OpenOptions::new().write(true).open(&target) {
        Ok(_) => false,
        Err(e) => {
            // PermissionDenied (os error 32: ERROR_SHARING_VIOLATION or 5: ACCESS_DENIED)
            e.kind() == std::io::ErrorKind::PermissionDenied
                || e.raw_os_error() == Some(32)
                || e.raw_os_error() == Some(5)
        }
    }
}

impl eframe::App for InstallerApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.08, 0.08, 0.09, 1.0]
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        ui.visuals_mut().override_text_color = Some(Color32::from_gray(240));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_rgb(20, 20, 22)).inner_margin(24.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Top branding header
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Sad Launcher")
                                .font(egui::FontId::proportional(18.0))
                                .strong()
                                .color(Color32::from_gray(250)),
                        );
                        ui.label(
                            RichText::new("Мастер быстрой установки")
                                .font(egui::FontId::proportional(11.0))
                                .color(Color32::from_gray(140)),
                        );
                    });
                });

                ui.add_space(18.0);
                ui.painter().line_segment(
                    [
                        Pos2::new(ui.cursor().left(), ui.cursor().top()),
                        Pos2::new(ui.cursor().left() + ui.available_width(), ui.cursor().top()),
                    ],
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 18)),
                );
                ui.add_space(16.0);

                match self.step.clone() {
                    Step::Config => self.view_config(ui),
                    Step::Installing => self.view_installing(ui),
                    Step::Done => self.view_done(ui),
                    Step::Error(err) => self.view_error(ui, &err),
                }
            });
    }
}

impl InstallerApp {
    fn view_config(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new("Папка установки:")
                .font(egui::FontId::proportional(13.0))
                .color(Color32::from_gray(220)),
        );
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            let mut dir_str = self.install_dir.display().to_string();
            let edit = ui.add(
                egui::TextEdit::singleline(&mut dir_str)
                    .desired_width(ui.available_width() - 95.0)
                    .font(egui::FontId::monospace(11.5))
                    .margin(egui::Margin::symmetric(10, 8)),
            );
            if edit.changed() {
                self.install_dir = PathBuf::from(dir_str);
                self.process_running = is_launcher_running(&self.install_dir);
            }

            if ui
                .add(
                    egui::Button::new(
                        RichText::new("Обзор…")
                            .font(egui::FontId::proportional(12.5))
                            .color(Color32::from_gray(230)),
                    )
                    .min_size(Vec2::new(82.0, 32.0)),
                )
                .clicked()
            {
                if let Some(picked) = FileDialog::new()
                    .set_title("Выберите папку для Sad Launcher")
                    .set_directory(&self.install_dir)
                    .pick_folder()
                {
                    self.install_dir = picked;
                    self.process_running = is_launcher_running(&self.install_dir);
                }
            }
        });
        ui.add_space(14.0);

        // Options
        ui.checkbox(
            &mut self.shortcut_desktop,
            RichText::new("Создать ярлык на рабочем столе").font(egui::FontId::proportional(13.0)),
        );
        ui.add_space(4.0);
        ui.checkbox(
            &mut self.shortcut_start,
            RichText::new("Добавить ярлык в меню «Пуск»").font(egui::FontId::proportional(13.0)),
        );
        ui.add_space(4.0);
        ui.checkbox(
            &mut self.launch_after,
            RichText::new("Запустить лаунчер после завершения установки")
                .font(egui::FontId::proportional(13.0)),
        );
        ui.add_space(16.0);

        // Process running warning card
        if self.process_running {
            let warn_frame = egui::Frame::new()
                .fill(Color32::from_rgba_unmultiplied(240, 160, 40, 15))
                .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(240, 160, 40, 60)))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(14, 10));

            warn_frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Лаунчер сейчас запущен. Закройте его, чтобы обновить.")
                            .font(egui::FontId::proportional(12.0))
                            .color(Color32::from_rgb(250, 190, 80)),
                    );
                    if ui
                        .add(egui::Button::new(
                            RichText::new("Проверить снова").font(egui::FontId::proportional(11.0)),
                        ))
                        .clicked()
                    {
                        self.process_running = is_launcher_running(&self.install_dir);
                    }
                });
            });
            ui.add_space(14.0);
        }

        // Install button
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            let btn_enabled = !self.process_running;
            let btn = ui.add_sized(
                Vec2::new(ui.available_width(), 40.0),
                egui::Button::new(
                    RichText::new("Установить Sad Launcher")
                        .font(egui::FontId::proportional(14.5))
                        .strong()
                        .color(if btn_enabled { Color32::BLACK } else { Color32::from_gray(120) }),
                )
                .fill(if btn_enabled { Color32::from_gray(245) } else { Color32::from_gray(50) })
                .corner_radius(CornerRadius::same(10)),
            );

            if btn.clicked() && btn_enabled {
                self.step = Step::Installing;
                self.progress = 0.05;
                self.progress_status = "Подготовка папки…".to_string();
                self.run_install();
            }
        });
    }

    fn run_install(&mut self) {
        let dest = self.install_dir.clone();
        if let Err(e) = std::fs::create_dir_all(&dest) {
            self.step = Step::Error(format!("Не удалось создать папку: {e}"));
            return;
        }

        let exe_target = dest.join("sad-launcher.exe");
        let vertdll_target = dest.join("vertdll.dll");

        // Write launcher binary
        self.progress = 0.35;
        self.progress_status = "Копирование sad-launcher.exe…".to_string();
        if let Err(e) = std::fs::write(&exe_target, PAYLOAD) {
            self.step = Step::Error(format!("Не удалось записать sad-launcher.exe: {e}"));
            return;
        }

        // The old Rust-edition proxy is not needed anymore.
        self.progress = 0.65;
        let _ = std::fs::remove_file(&vertdll_target);

        // Pre-0.7 install under the old name: shortcuts, Start-menu folder,
        // old exe. Data in %APPDATA% is moved by the launcher itself.
        let legacy = "LLM Launcher Madrid";
        let _ = std::fs::remove_file(desktop_dir().join(format!("{legacy}.lnk")));
        let _ = std::fs::remove_dir_all(programs_dir().join(legacy));
        let _ = std::fs::remove_file(dest.join("madrid-launcher.exe"));
        let old_dir = default_install_dir().with_file_name(legacy);
        if old_dir != dest {
            let _ = std::fs::remove_file(old_dir.join("madrid-launcher.exe"));
            let _ = std::fs::remove_dir(&old_dir);
        }

        // Create shortcuts
        self.progress = 0.85;
        self.progress_status = "Создание ярлыков…".to_string();

        if self.shortcut_desktop {
            let desk_link = desktop_dir().join("Sad Launcher.lnk");
            make_shortcut(&exe_target, &desk_link);
        }
        if self.shortcut_start {
            let prog_dir = programs_dir().join("Sad Launcher");
            let _ = std::fs::create_dir_all(&prog_dir);
            let start_link = prog_dir.join("Sad Launcher.lnk");
            make_shortcut(&exe_target, &start_link);
        }

        self.progress = 1.0;
        self.progress_status = "Установка завершена!".to_string();
        self.step = Step::Done;
    }

    fn view_installing(&mut self, ui: &mut Ui) {
        ui.add_space(20.0);
        ui.label(
            RichText::new(&self.progress_status)
                .font(egui::FontId::proportional(14.0))
                .color(Color32::from_gray(240)),
        );
        ui.add_space(14.0);

        let progress_rect = ui.allocate_exact_size(Vec2::new(ui.available_width(), 6.0), Sense::hover()).0;
        ui.painter().rect_filled(progress_rect, CornerRadius::same(3), Color32::from_gray(35));
        let fill_w = progress_rect.width() * self.progress.clamp(0.0, 1.0);
        let fill_rect = Rect::from_min_size(progress_rect.min, Vec2::new(fill_w, progress_rect.height()));
        ui.painter().rect_filled(fill_rect, CornerRadius::same(3), Color32::from_gray(245));
    }

    fn view_done(&mut self, ui: &mut Ui) {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            // Drawn check mark in a ring: the fonts have no «✓» glyph (it
            // showed as an empty box).
            let (r, _) = ui.allocate_exact_size(egui::Vec2::splat(34.0), egui::Sense::hover());
            let green = Color32::from_rgb(80, 220, 120);
            let c = r.center();
            ui.painter().circle_stroke(c, 15.0, egui::Stroke::new(1.6, green));
            ui.painter().add(egui::Shape::line(
                vec![
                    egui::pos2(c.x - 6.5, c.y + 0.5),
                    egui::pos2(c.x - 2.0, c.y + 5.0),
                    egui::pos2(c.x + 7.0, c.y - 5.0),
                ],
                egui::Stroke::new(2.2, green),
            ));
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Установка успешно завершена!")
                        .font(egui::FontId::proportional(17.0))
                        .strong()
                        .color(Color32::from_gray(250)),
                );
                ui.label(
                    RichText::new("Sad Launcher готов к запуску.")
                        .font(egui::FontId::proportional(12.5))
                        .color(Color32::from_gray(160)),
                );
            });
        });

        ui.add_space(24.0);
        ui.label(
            RichText::new(format!("Папка: {}", self.install_dir.display()))
                .font(egui::FontId::monospace(10.5))
                .color(Color32::from_gray(130)),
        );

        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.horizontal(|ui| {
                let launch = ui.add_sized(
                    Vec2::new(ui.available_width() - 110.0, 38.0),
                    egui::Button::new(
                        RichText::new("Запустить лаунчер")
                            .font(egui::FontId::proportional(14.0))
                            .strong()
                            .color(Color32::BLACK),
                    )
                    .fill(Color32::from_gray(245))
                    .corner_radius(CornerRadius::same(10)),
                );

                if launch.clicked() {
                    let exe = self.install_dir.join("sad-launcher.exe");
                    let _ = std::process::Command::new(exe).spawn();
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }

                if ui
                    .add_sized(
                        Vec2::new(100.0, 38.0),
                        egui::Button::new(
                            RichText::new("Закрыть")
                                .font(egui::FontId::proportional(13.0))
                                .color(Color32::from_gray(220)),
                        )
                        .corner_radius(CornerRadius::same(10)),
                    )
                    .clicked()
                {
                    if self.launch_after {
                        let exe = self.install_dir.join("sad-launcher.exe");
                        let _ = std::process::Command::new(exe).spawn();
                    }
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }

    fn view_error(&mut self, ui: &mut Ui, err: &str) {
        ui.add_space(10.0);
        ui.label(
            RichText::new("Произошла ошибка при установке:")
                .font(egui::FontId::proportional(15.0))
                .color(Color32::from_rgb(240, 80, 80)),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new(err)
                .font(egui::FontId::monospace(11.5))
                .color(Color32::from_gray(200)),
        );

        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            if ui
                .add_sized(
                    Vec2::new(ui.available_width(), 38.0),
                    egui::Button::new(
                        RichText::new("Вернуться назад")
                            .font(egui::FontId::proportional(13.5))
                            .color(Color32::from_gray(240)),
                    )
                    .corner_radius(CornerRadius::same(10)),
                )
                .clicked()
            {
                self.step = Step::Config;
            }
        });
    }
}

fn window_icon() -> egui::IconData {
    match image::load_from_memory(include_bytes!("../../assets/icon.png")) {
        Ok(i) => {
            let i = i.into_rgba8();
            egui::IconData { width: i.width(), height: i.height(), rgba: i.into_raw() }
        }
        Err(_) => egui::IconData::default(),
    }
}

pub fn run_silent_update(target_exe: PathBuf) {
    // Wait up to 60 s for the running launcher to exit (it stops its hosted
    // Minecraft server first) and release its file lock.
    let mut replaced = false;
    let mut last_err = String::new();
    for _ in 0..400 {
        match std::fs::write(&target_exe, PAYLOAD) {
            Ok(_) => {
                replaced = true;
                break;
            }
            Err(e) => {
                last_err = e.to_string();
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
        }
    }
    if !replaced {
        // Fallback: open the normal installer UI pre-pointed at the target folder so the user can retry.
        let mut fallback = InstallerApp::default();
        if let Some(parent) = target_exe.parent() {
            fallback.install_dir = parent.to_path_buf();
        }
        fallback.step = Step::Error(format!(
            "Не удалось заменить файл лаунчера ({last_err}). Закройте Sad Launcher и нажмите «Вернуться назад»."
        ));
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Обновление Sad Launcher")
                .with_inner_size([540.0, 360.0])
                .with_resizable(false)
                .with_icon(std::sync::Arc::new(window_icon())),
            ..Default::default()
        };
        let _ = eframe::run_native(
            "Sad Launcher — Обновление",
            options,
            Box::new(|_cc| Ok(Box::new(fallback))),
        );
        return;
    }

    if let Some(dir) = target_exe.parent() {
        let _ = std::fs::remove_file(dir.join("vertdll.dll"));
        let _ = std::fs::remove_file(dir.join("madrid-launcher.exe"));
    }
    let _ = std::process::Command::new(&target_exe).spawn();
}

pub fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Установка Sad Launcher")
            .with_inner_size([540.0, 360.0])
            .with_min_inner_size([500.0, 340.0])
            .with_resizable(false)
            .with_icon(std::sync::Arc::new(window_icon())),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Sad Launcher — Установка",
        options,
        Box::new(|_cc| Ok(Box::new(InstallerApp::default()))),
    );
}
}
