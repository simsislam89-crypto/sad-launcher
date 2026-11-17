import io

# ============ server view: friends code card
p = "src/views/server.rs"
src = io.open(p, encoding="utf-8").read()

src = src.replace('''        let mut start = false;
        let mut stop = false;
        let mut connect = false;''', '''        let mut start = false;
        let mut stop = false;
        let mut connect = false;
        let mut copy_code = false;
        let mut join_friend = false;
        let my_code = self
            .public_ip(ui.ctx())
            .map(|ip| format!("{}:{}", ip, cfg_port));''')

friends_card = '''
        // ------------------------------------------------------ friends code
        ui.add_space(14.0);
        let frame = theme::card_frame(skin);
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width() - 32.0);
            widgets::section_label(ui, skin, "ДРУЗЬЯ ПО КОДУ");
            ui.add_space(6.0);

            ui.horizontal(|ui| {
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
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Друг вставляет этот код у себя и жмёт «Зайти к другу». Работает через проброс порта или Radmin VPN / Hamachi — там в коде IP вашей виртуальной сети.",
                )
                .font(theme::body(11.0))
                .color(skin.faint()),
            );
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Код друга")
                        .font(theme::body(12.5))
                        .color(skin.dim()),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, skin, "Зайти к другу", Btn::Primary, true).clicked() {
                        join_friend = true;
                    }
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.friend_code)
                            .id_salt("friend-code")
                            .hint_text(
                                RichText::new("1.2.3.4:25565").color(skin.faint()),
                            )
                            .desired_width(160.0)
                            .font(theme::mono(12.0))
                            .text_color(skin.ink())
                            .margin(egui::Margin::symmetric(6, 4)),
                    );
                    let _ = resp;
                });
            });
        });
'''

anchor = "        // ---------------------------------------------------------- console"
assert anchor in src
src = src.replace(anchor, friends_card + anchor, 1)

# handlers
src = src.replace('''        if copy_addr {
            ui.ctx().copy_text(format!("127.0.0.1:{cfg_port}"));
            self.toast("Адрес скопирован");
        }''', '''        if copy_addr {
            ui.ctx().copy_text(format!("127.0.0.1:{cfg_port}"));
            self.toast("Адрес скопирован");
        }
        if copy_code {
            if let Some(code) = &my_code {
                ui.ctx().copy_text(code.clone());
                self.toast("Код для друга скопирован");
            } else {
                self.toast("IP ещё не определился — секунду");
            }
        }
        if join_friend {
            let code = self.friend_code.trim().to_string();
            let ok = {
                let mut parts = code.split(':');
                let ip = parts.next().unwrap_or("").trim();
                let port = parts.next().unwrap_or("").trim();
                !ip.is_empty()
                    && port.parse::<u16>().map(|p| p > 0).unwrap_or(false)
            };
            if ok {
                self.launch_to(Some(&code));
            } else {
                self.toast("Код должен быть вида IP:порт");
            }
        }''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)

# ============ mods.rs: drop unused pick_source
p = "src/mods.rs"
src = io.open(p, encoding="utf-8").read()
start = src.find("pub fn pick_source")
if start != -1:
    end = src.find("}\n\n", start)
    src = src[:start] + src[end + 3:]
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

print("friends ok")
