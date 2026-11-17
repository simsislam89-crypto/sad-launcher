import io

# ================= mods.rs: pack ownership records + group helper =================
p = "src/mods.rs"
src = io.open(p, encoding="utf-8").read()

records = '''
// ---------------------------------------------------------------- pack ownership

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct PackRecord {
    pub pack: String,
    pub files: Vec<String>,
}

fn packs_db() -> PathBuf {
    crate::config::data_dir().join("meta").join("packs").join("installed.json")
}

pub fn load_pack_records() -> Vec<PackRecord> {
    std::fs::read_to_string(packs_db())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_pack_records(records: &[PackRecord]) {
    let path = packs_db();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(records) {
        let _ = std::fs::write(path, text);
    }
}

/// Remembers which files a modpack brought in, so «Установленные»
/// can group mods by pack instead of one messy pile.
pub fn record_pack_files(pack_title: &str, files: &[String]) {
    let mut records = load_pack_records();
    let entry = records
        .iter_mut()
        .find(|r| r.pack == pack_title)
        .unwrap_or_else(|| {
            records.push(PackRecord {
                pack: pack_title.to_string(),
                files: Vec::new(),
            });
            records.last_mut().unwrap()
        });
    for f in files {
        if !entry.files.contains(f) {
            entry.files.push(f.clone());
        }
    }
    save_pack_records(&records);
}

pub fn forget_pack(pack_title: &str) {
    let mut records = load_pack_records();
    records.retain(|r| r.pack != pack_title);
    save_pack_records(&records);
}

/// Mods that came from no pack.
pub fn ungrouped_files(records: &[PackRecord]) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for r in records {
        for f in &r.files {
            out.insert(f.clone());
        }
    }
    out
}
'''
anchor = "fn sanitize(name: &str) -> String {"
assert anchor in src
src = src.replace(anchor, records.strip() + "\n\n" + anchor, 1)

# record files when a modpack installs (mrpack path)
src = src.replace('''    let overrides = unpacked.join(index.overrides.as_deref().unwrap_or("overrides"));
    if overrides.is_dir() {
        let _ = copy_dir(&overrides, &game)?;
    }
    on(ModEvent::Progress {
        done: total,
        total,
    });
    Ok(game.join("mods"))
}''', '''    let overrides = unpacked.join(index.overrides.as_deref().unwrap_or("overrides"));
    if overrides.is_dir() {
        let _ = copy_dir(&overrides, &game)?;
    }
    let owned: Vec<String> = index
        .files
        .iter()
        .map(|f| {
            f.path
                .rsplit('/')
                .next()
                .unwrap_or(&f.path)
                .to_string()
        })
        .collect();
    record_pack_files(&hit.title, &owned);
    on(ModEvent::Progress {
        done: total,
        total,
    });
    Ok(game.join("mods"))
}''')

# CF pack path
src = src.replace('''    let overrides = unpacked.join(&manifest.overrides);
    if overrides.is_dir() {
        copy_dir(&overrides, &cfg.game_dir())?;
    }
    on(ModEvent::Progress {
        done: total,
        total,
    });
    Ok(mods_dir)
}''', '''    let overrides = unpacked.join(&manifest.overrides);
    if overrides.is_dir() {
        copy_dir(&overrides, &cfg.game_dir())?;
    }
    let _ = hit.title;
    on(ModEvent::Progress {
        done: total,
        total,
    });
    Ok(mods_dir)
}''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("pack records ok")

# ================= views/mods.rs: groups + delete all =================
p = "src/views/mods.rs"
src = io.open(p, encoding="utf-8").read()

src = src.replace('''        let mut clear_build = false;''', '''        let mut clear_build = false;
        let mut delete_all = false;
        let mut delete_group: Option<String> = None;''')

# replace the installed list rendering with grouped sections
old_installed = '''                    widgets::section_label(ui, skin, "УСТАНОВЛЕННЫЕ МОДЫ");
                    ui.add_space(4.0);
                    if installed.is_empty() {
                        widgets::empty_state(
                            ui,
                            skin,
                            Icon::Plus,
                            "Пока пусто",
                            "Открой «Каталог» и добавь моды — они лягут в папку mods.",
                        );
                    } else {
                        for m in &installed {
                            let (toggle, delete) = mod_row(
                                ui,
                                skin,
                                &m.name,
                                &m.file,
                                m.size,
                                m.enabled,
                                confirm.as_deref() == Some(m.file.as_str()),
                            );
                            if toggle {
                                toggle_req = Some((m.file.clone(), !m.enabled));
                            }
                            if delete {
                                if confirm.as_deref() == Some(m.file.as_str()) {
                                    delete_req = Some(m.file.clone());
                                    confirm = None;
                                } else {
                                    confirm = Some(m.file.clone());
                                }
                            }
                        }
                    }
                    ui.add_space(12.0);'''
new_installed = '''                    widgets::section_label(ui, skin, "УСТАНОВЛЕННОЕ");
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if widgets::button_sized(
                            ui,
                            skin,
                            "Удалить все моды",
                            Btn::Outline,
                            !installed.is_empty(),
                            26.0,
                            11.0,
                            10.5,
                        )
                        .clicked()
                        {
                            delete_all = true;
                        }
                        ui.label(
                            RichText::new(format!("всего: {}", installed.len()))
                                .font(theme::mono(9.5))
                                .color(skin.faint()),
                        );
                    });
                    ui.add_space(8.0);

                    if installed.is_empty() {
                        widgets::empty_state(
                            ui,
                            skin,
                            Icon::Plus,
                            "Пока пусто",
                            "Открой «Каталог» и добавь моды — они лягут в папку mods.",
                        );
                    } else {
                        let records = mods::load_pack_records();
                        let mut remaining: Vec<_> = installed.clone();
                        for rec in &records {
                            let group: Vec<_> = remaining
                                .iter()
                                .filter(|m| rec.files.iter().any(|f| f == &m.file || m.file.contains(f.trim_end_matches(".jar"))))
                                .cloned()
                                .collect();
                            if group.is_empty() {
                                continue;
                            }
                            for g in &group {
                                remaining.retain(|m| m.file != g.file);
                            }
                            widgets::section_label(ui, skin, &format!("МОДПАК · {}", rec.pack));
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                if widgets::button_sized(
                                    ui,
                                    skin,
                                    &format!("Удалить пак ({} модов)", group.len()),
                                    Btn::Ghost,
                                    true,
                                    24.0,
                                    10.0,
                                    9.5,
                                )
                                .clicked()
                                {
                                    delete_group = Some(rec.pack.clone());
                                }
                            });
                            ui.add_space(4.0);
                            for m in &group {
                                let (toggle, delete) = mod_row(
                                    ui,
                                    skin,
                                    &m.name,
                                    &m.file,
                                    m.size,
                                    m.enabled,
                                    confirm.as_deref() == Some(m.file.as_str()),
                                );
                                if toggle {
                                    toggle_req = Some((m.file.clone(), !m.enabled));
                                }
                                if delete {
                                    if confirm.as_deref() == Some(m.file.as_str()) {
                                        delete_req = Some(m.file.clone());
                                        confirm = None;
                                    } else {
                                        confirm = Some(m.file.clone());
                                    }
                                }
                            }
                            ui.add_space(10.0);
                        }

                        widgets::section_label(ui, skin, "ОТДЕЛЬНЫЕ МОДЫ");
                        ui.add_space(2.0);
                        if remaining.is_empty() {
                            ui.label(
                                RichText::new("Все моды принадлежат пакам.")
                                    .font(theme::mono(9.5))
                                    .color(skin.faint()),
                            );
                        }
                        for m in &remaining {
                            let (toggle, delete) = mod_row(
                                ui,
                                skin,
                                &m.name,
                                &m.file,
                                m.size,
                                m.enabled,
                                confirm.as_deref() == Some(m.file.as_str()),
                            );
                            if toggle {
                                toggle_req = Some((m.file.clone(), !m.enabled));
                            }
                            if delete {
                                if confirm.as_deref() == Some(m.file.as_str()) {
                                    delete_req = Some(m.file.clone());
                                    confirm = None;
                                } else {
                                    confirm = Some(m.file.clone());
                                }
                            }
                        }
                    }
                    ui.add_space(12.0);'''
assert old_installed in src, "installed block not found"
src = src.replace(old_installed, new_installed, 1)

# handlers
src = src.replace('''        if clear_build {''', '''        if delete_all {
            let mut ok = 0;
            let mut bad = 0;
            for m in self.mods_cache.clone() {
                match mods::delete(&self.cfg, &m.file) {
                    Ok(_) => ok += 1,
                    Err(_) => bad += 1,
                }
            }
            mods::save_build(&self.build);
            self.toast(format!("Удалено модов: {ok}{}", if bad > 0 { format!(" · не удалось: {bad}") } else { String::new() }));
        }
        if let Some(pack) = delete_group {
            let records = mods::load_pack_records();
            if let Some(rec) = records.iter().find(|r| r.pack == pack) {
                let mut ok = 0;
                for f in &rec.files {
                    if mods::delete(&self.cfg, f).is_ok() {
                        ok += 1;
                    }
                }
                mods::forget_pack(&pack);
                self.toast(format!("Пак удалён · файлов: {ok}"));
            }
        }
        if clear_build {''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("groups ok")
