import io

# ================= views/mods.rs: clip card content, shorter text, wrapping =================
p = "src/views/mods.rs"
src = io.open(p, encoding="utf-8").read()

# hit_card: title/meta shortened + clip the painter to the cell
src = src.replace('''    // title + meta
    let ty = irect.bottom() + 8.0;
    ui.painter().text(
        Pos2::new(rect.left() + 10.0, ty + 8.0),
        Align2::LEFT_CENTER,
        truncate(&hit.title, 26),
        theme::body(12.5),
        skin.ink(),
    );
    ui.painter().text(
        Pos2::new(rect.left() + 10.0, ty + 26.0),
        Align2::LEFT_CENTER,
        format!("{} · {}", downloads_label(hit.downloads),
            if hit.source == "curseforge" { "CF" } else { "Modrinth" }),
        theme::mono(8.5),
        skin.faint(),
    );''', '''    // title + meta: clipped to the cell so nothing leaks outside it
    let ty = irect.bottom() + 8.0;
    let painter = ui.painter().with_clip_rect(rect);
    painter.text(
        Pos2::new(rect.left() + 10.0, ty + 8.0),
        Align2::LEFT_CENTER,
        truncate(&hit.title, 18),
        theme::body(12.5),
        skin.ink(),
    );
    painter.text(
        Pos2::new(rect.left() + 10.0, ty + 26.0),
        Align2::LEFT_CENTER,
        format!("{} · {}", downloads_label(hit.downloads),
            if hit.source == "curseforge" { "CF" } else { "Modrinth" }),
        theme::mono(8.5),
        skin.faint(),
    );''')

# catalog hint: wrap long lines
src = src.replace('''            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(match kind {
                        mods::KIND_MAP => {''', '''            ui.horizontal(|ui| {
                ui.add(egui::Label::new(
                    RichText::new(match kind {
                        mods::KIND_MAP => {''')
src = src.replace('''                    })
                    .font(theme::mono(9.0))
                    .color(skin.faint()),
                );
            });
            ui.add_space(10.0);

            if let Some(err) = self.search.error.clone() {''', '''                    })
                    .font(theme::mono(9.0))
                    .color(skin.faint())
                ).wrap());
            });
            ui.add_space(10.0);

            if let Some(err) = self.search.error.clone() {''')

# search error card: wrap the error text
src = src.replace('''                ui.label(
                    RichText::new(truncate(&err, 240))
                        .font(theme::body(12.0))
                        .color(skin.dim()),
                );''', '''                ui.add(
                    egui::Label::new(
                        RichText::new(truncate(&err, 240))
                            .font(theme::body(12.0))
                            .color(skin.dim()),
                    )
                    .wrap(),
                );''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("mods text ok")

# ================= views/server.rs: wrap long hint paragraphs =================
p = "src/views/server.rs"
src = io.open(p, encoding="utf-8").read()
for marker in [
    '"Друг вставляет этот код у себя и жмёт «Зайти к другу». Работает через проброс порта или Radmin VPN / Hamachi — там в коде IP вашей виртуальной сети.",',
    '"У друга должна быть та же сборка модов — иначе Forge не пустит на сервер.",',
]:
    src = src.replace(
        f'''            ui.label(
                RichText::new(
                    {marker}
                )
                .font(theme::body(11.0))
                .color(skin.faint()),
            );''',
        f'''            ui.add(
                egui::Label::new(
                    RichText::new(
                        {marker}
                    )
                    .font(theme::body(11.0))
                    .color(skin.faint()),
                )
                .wrap(),
            );''',
    )
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("server text ok")
