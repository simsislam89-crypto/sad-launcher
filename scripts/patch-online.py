import io

# ================= server.rs: public vanilla server helper =================
p = "src/server.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''/// Boots `java` with no console and pipes its output into the launcher.''', '''/// Ensures a vanilla server of the given MC version in `dir` (jar + eula +
/// properties). Used by the Rust edition, which only speaks up to 1.18.2.
pub fn ensure_vanilla_server_dir(
    shared: &Arc<Shared>,
    cfg: &Config,
    dir: &Path,
    mc: &str,
    port: u16,
) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    minecraft::prepare_server_dir(dir, port);
    let jar = dir.join("minecraft_server.jar");
    if !jar.exists() {
        shared.set_mc_stage(format!("Скачиваю vanilla {mc}…"));
        let client = minecraft::http_client()?;
        tokio_block(download_vanilla_server(&client, mc, dir))?;
    }
    write_marker(dir, mc, "vanilla", "vanilla");
    let _ = cfg;
    Ok(())
}

/// Boots a detached vanilla server and waits until its port accepts TCP.
/// Returns the bound address once the game can connect.
pub fn start_detached_vanilla(
    shared: &Arc<Shared>,
    java: &crate::java::JavaInfo,
    dir: &Path,
    port: u16,
) -> Result<String> {
    let mut cmd = Command::new(&java.path);
    cmd.args([
        "-Xmx2048M",
        "-jar".into(),
        dir.join("minecraft_server.jar").display().to_string(),
        "nogui".into(),
    ]);
    cmd.current_dir(dir);
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let child = cmd.spawn().context("Vanilla-сервер не запустился")?;
    shared.push_mc(format!(
        "[Madrid] vanilla-сервер запущен на порту {port} (для Rust-издания)"
    ));
    drop(child);

    // Wait until the server accepts connections on its port.
    for _ in 0..180 {
        if std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            std::time::Duration::from_millis(300),
        )
        .is_ok()
        {
            return Ok(format!("127.0.0.1:{port}"));
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    bail!("Сервер не открыл порт {port} за 90 секунд");
}

/// Boots `java` with no console and pipes its output into the launcher.''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("server vanilla ok")

# ================= rustmc.rs: vanilla 1.18.2 server before launch =================
p = "src/rustmc.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''    let mut cmd = Command::new(&exe);
    cmd.arg("--username".to_string()).arg(cfg.nickname_or_default());
    if shared.is_mc_running() {
        cmd.arg("--server")
            .arg(format!("127.0.0.1:{}", cfg.server_port));
    }''', '''    // stevenarella speaks up to 1.18.2 (protocol 758), so it gets its own
    // vanilla 1.18.2 server on port server_port + 1, never the Forge one.
    let port = cfg.server_port.saturating_add(1).max(1024);
    let dir = crate::config::data_dir().join("rustmc").join("server");
    let java = crate::java::pick_for(&cfg.java_path, 17)
        .ok_or_else(|| anyhow::anyhow!("Для Rust-издания нужна Java 17 — скачай её"))?;
    crate::server::ensure_vanilla_server_dir(shared, cfg, &dir, "1.18.2", port)?;
    let addr = crate::server::start_detached_vanilla(shared, &java, &dir, port)?;
    shared.push_game(format!("[Madrid] vanilla 1.18.2 · {addr}"));

    let mut cmd = Command::new(&exe);
    cmd.arg("--username".to_string()).arg(cfg.nickname_or_default());
    cmd.arg("--server").arg(addr);''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("rustmc vanilla ok")

# ================= shared.rs: tail-only log clones =================
p = "src/shared.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''    pub fn game_log(&self) -> Vec<String> {
        self.game_log
            .lock()
            .map(|l| l.iter().cloned().collect())
            .unwrap_or_default()
    }''', '''    pub fn game_log(&self) -> Vec<String> {
        self.game_log
            .lock()
            .map(|l| l.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Only the last `n` lines — cheap for per-frame UI reads.
    pub fn game_log_tail(&self, n: usize) -> Vec<String> {
        self.game_log
            .lock()
            .map(|l| l.iter().rev().take(n).cloned().rev().collect())
            .unwrap_or_default()
    }''')
src = src.replace('''    pub fn mc_log(&self) -> Vec<String> {
        self.mc_log
            .lock()
            .map(|l| l.iter().cloned().collect())
            .unwrap_or_default()
    }''', '''    pub fn mc_log(&self) -> Vec<String> {
        self.mc_log
            .lock()
            .map(|l| l.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn mc_log_tail(&self, n: usize) -> Vec<String> {
        self.mc_log
            .lock()
            .map(|l| l.iter().rev().take(n).cloned().rev().collect())
            .unwrap_or_default()
    }''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

# use tails in views
p = "src/views/play.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace("let lines = self.shared.game_log();", "let lines = self.shared.game_log_tail(240);")
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

p = "src/views/server.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace("let logs = self.shared.mc_log();", "let logs = self.shared.mc_log_tail(240);")
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("perf tails ok")

# ================= views/server.rs: porthole-like port -> address =================
p = "src/views/server.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''        let my_code = self
            .public_ip(ui.ctx())
            .map(|ip| format!("{}:{}", ip, cfg_port));''', '''        let mut port_buf = self.friend_port.clone();
        if port_buf.is_empty() {
            port_buf = cfg_port.to_string();
        }
        let my_code = self
            .public_ip(ui.ctx())
            .map(|ip| format!("{}:{}", ip, port_buf.trim()));''')

# port input + address preview inside friends card
src = src.replace('''            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Твой код (IP:порт)")
                        .font(theme::body(12.5))
                        .color(skin.dim()),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let code = match &my_code {
                        Some(c) => c.clone(),
                        None => "определяю IP…".to_string(),
                    };
                    if widgets::chip(ui, skin, &code, running).clicked() {
                        copy_code = true;
                    }
                });
            });''', '''            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Порт для друзей")
                        .font(theme::body(12.5))
                        .color(skin.dim()),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.friend_port)
                            .id_salt("friend-port")
                            .hint_text(RichText::new("25565").color(skin.faint()))
                            .desired_width(70.0)
                            .font(theme::mono(12.0))
                            .text_color(skin.ink())
                            .margin(egui::Margin::symmetric(6, 4)),
                    );
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Готовый адрес сервера")
                        .font(theme::body(12.5))
                        .color(skin.dim()),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let code = match &my_code {
                        Some(c) => c.clone(),
                        None => "определяю IP…".to_string(),
                    };
                    if widgets::chip(ui, skin, &code, running).clicked() {
                        copy_code = true;
                    }
                });
            });''')

src = src.replace('''        let mut copy_code = false;
        let mut copy_list = false;''', '''        let mut copy_code = false;
        let mut copy_list = false;
        let _ = &mut port_buf;''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("porthole ui ok")

# app.rs: friend_port field
p = "src/app.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace("    pub friend_code: String,", "    pub friend_code: String,\n    pub friend_port: String,")
src = src.replace("            friend_code: String::new(),", "            friend_code: String::new(),\n            friend_port: String::new(),")
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("friend_port ok")
