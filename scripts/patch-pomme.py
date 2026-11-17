import io

# ================= rustmc.rs: Pomme install/launch =================
p = "src/rustmc.rs"
src = io.open(p, encoding="utf-8").read()

src = src.replace('''pub fn installed() -> bool {
    exe_path().is_file()
}''', '''pub fn installed() -> bool {
    exe_path().is_file()
}

// ------------------------------------------------------------- Pomme
// A native Rust+Vulkan client with an embedded singleplayer server.
// Ships as one self-contained exe — no Java, no login screen.

pub const POMME_LATEST: &str = "https://api.github.com/repos/PommeMC/Client/releases/latest";

fn pomme_dir() -> PathBuf {
    dir().join("pomme")
}

pub fn pomme_exe() -> PathBuf {
    pomme_dir().join("pomme-client.exe")
}

pub fn pomme_installed() -> bool {
    pomme_exe().is_file()
}

/// Downloads the latest portable Pomme build and unpacks the exe.
pub fn install_pomme(tx: &std::sync::mpsc::Sender<RustEvent>) -> Result<()> {
    let _ = std::fs::create_dir_all(pomme_dir());
    let _ = tx.send(RustEvent::Stage("Ищу свежий релиз Pomme…".into()));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Не удалось создать runtime")?;
    let zip_path = pomme_dir().join("pomme.zip");
    rt.block_on(async {
        let client = crate::minecraft::http_client()?;
        #[derive(serde::Deserialize)]
        struct Release {
            assets: Vec<Asset>,
        }
        #[derive(serde::Deserialize)]
        struct Asset {
            name: String,
            #[serde(rename = "browser_download_url")]
            url: String,
        }
        let rel: Release = client
            .get(POMME_LATEST)
            .header("User-Agent", crate::minecraft::user_agent())
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await
            .context("GitHub не отвечает")?
            .error_for_status()?
            .json()
            .await
            .context("Список релизов не читается")?;
        let asset = rel
            .assets
            .into_iter()
            .find(|a| a.name == "pomme-client-windows-x64.zip")
            .ok_or_else(|| anyhow::anyhow!("В релизе нет Windows-сборки"))?;
        let _ = tx.send(RustEvent::Stage("Скачиваю Pomme (~32 МБ)…".into()));
        let resp = client
            .get(asset.url)
            .header("User-Agent", crate::minecraft::user_agent())
            .timeout(std::time::Duration::from_secs(600))
            .send()
            .await
            .context("Pomme не скачался")?
            .error_for_status()?;
        let mut file = std::fs::File::create(&zip_path)?;
        let bytes = resp.bytes().await?;
        std::io::Write::write_all(&mut file, &bytes)?;
        Ok::<(), anyhow::Error>(())
    })?;
    unzip_archive(&zip_path, &pomme_dir())?;
    let _ = std::fs::remove_file(&zip_path);
    if !pomme_installed() {
        bail!("После распаковки pomme-client.exe не нашёлся");
    }
    let _ = tx.send(RustEvent::Done);
    Ok(())
}

/// Pomme is self-contained: launch it directly, singleplayer without login.
pub fn launch_pomme(shared: &Arc<Shared>) -> Result<()> {
    let exe = pomme_exe();
    if !exe.is_file() {
        bail!("Pomme не установлен");
    }
    let mut cmd = Command::new(&exe);
    cmd.current_dir(pomme_dir());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.spawn().context("Pomme не запустился")?;
    shared.log(
        crate::shared::LogKind::Info,
        "Pomme запущен (Rust + Vulkan, без Java)",
    );
    Ok(())
}''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("pomme functions ok")

# ================= versions.rs: Pomme row in the Rust card =================
p = "src/views/versions.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''                                } else if ready {
                                    if widgets::button_sized(ui, skin, "Играть", Btn::Primary, true, 28.0, 11.5, 11.0).clicked() {
                                        play_rust = true;
                                    }
                                    if widgets::button_sized(ui, skin, "Удалить", Btn::Ghost, true, 28.0, 11.5, 10.0).clicked() {
                                        delete_rust = true;
                                    }
                                } else {''', '''                                } else if ready {
                                    if widgets::button_sized(ui, skin, "Играть", Btn::Primary, true, 28.0, 11.5, 11.0).clicked() {
                                        play_rust = true;
                                    }
                                    if widgets::button_sized(ui, skin, "Удалить", Btn::Ghost, true, 28.0, 11.5, 10.0).clicked() {
                                        delete_rust = true;
                                    }
                                } else {''')

# add Pomme sub-row right after the rust card frame closes
src = src.replace('''                ui.add_space(16.0);
                widgets::section_label(ui, skin, "УСТАНОВЛЕНЫ");''', '''                // Pomme: native Rust + Vulkan, singleplayer without Java.
                ui.add_space(8.0);
                {
                    let pomme_ready = crate::rustmc::pomme_installed();
                    let building = self.rust_task.as_ref().map(|t| !t.finished).unwrap_or(false);
                    ui.horizontal(|ui| {
                        widgets::status_dot(
                            ui,
                            if pomme_ready { skin.ink() } else { skin.faint() },
                            pomme_ready,
                        );
                        ui.label(
                            RichText::new("Pomme — Rust + Vulkan, встроенный сервер, без Java")
                                .font(theme::body(12.0))
                                .color(skin.dim()),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if pomme_ready {
                                if widgets::button_sized(ui, skin, "Играть", Btn::Primary, true, 26.0, 11.0, 10.5).clicked() {
                                    play_pomme = true;
                                }
                            } else if widgets::button_sized(ui, skin, "Скачать Pomme (32 МБ)", Btn::Outline, !building, 26.0, 11.0, 10.0).clicked() {
                                install_pomme = true;
                            }
                        });
                    });
                }
                ui.add_space(16.0);
                widgets::section_label(ui, skin, "УСТАНОВЛЕНЫ");''')

src = src.replace("        let mut delete_rust = false;",
                  "        let mut delete_rust = false;\n        let mut install_pomme = false;\n        let mut play_pomme = false;")

src = src.replace('''        if play_rust {
            self.launch_rustmc();
        }''', '''        if play_rust {
            self.launch_rustmc();
        }
        if install_pomme {
            self.start_pomme_install(ui.ctx());
        }
        if play_pomme {
            match crate::rustmc::launch_pomme(&self.shared) {
                Ok(_) => self.toast("Pomme запускается — Rust, без Java"),
                Err(e) => self.toast(format!("{e:#}")),
            }
        }''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("versions pomme row ok")
