import io

# 1) rustmc.rs: pomme offline launch (no login screen)
p = "src/rustmc.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''    let mut cmd = Command::new(&exe);
    cmd.current_dir(pomme_dir());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.spawn().context("Pomme не запустился")?;''', '''    // Offline session: skips the Microsoft login screen entirely.
    let nickname = crate::config::load().nickname_or_default();
    let uuid = crate::minecraft::offline_uuid(&nickname);
    let mut cmd = Command::new(&exe);
    cmd.args([
        "--username".to_string(),
        nickname,
        "--uuid".to_string(),
        uuid,
        "--access-token".to_string(),
        "0".to_string(),
        "--game-dir".to_string(),
        pomme_dir().join("game").display().to_string(),
    ]);
    cmd.current_dir(pomme_dir());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.spawn().context("Pomme не запустился")?;''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("pomme offline ok")

# 2) profiles.rs: delete all modpack profiles
p = "src/profiles.rs"
src = io.open(p, encoding="utf-8").read()
src += '''
/// Removes every Modpack profile: its isolated mods storage is deleted too.
/// The active profile falls back to the default one.
pub fn delete_all_modpack_profiles(cfg: &mut Config) -> Result<u32> {
    let profiles = load_profiles();
    let mut removed = 0u32;
    let mut was_active = false;
    for p in &profiles {
        if p.kind == ProfileKind::Modpack {
            let storage = profile_mods_dir(&p.id);
            let _ = std::fs::remove_dir_all(storage);
            let _ = std::fs::remove_dir_all(profiles_dir().join(&p.id));
            if cfg.active_profile == p.id {
                was_active = true;
            }
            removed += 1;
        }
    }
    if was_active {
        switch_profile(cfg, "default")?;
    }
    let kept: Vec<Profile> = profiles
        .into_iter()
        .filter(|p| p.kind != ProfileKind::Modpack)
        .collect();
    save_profiles(&kept)?;
    Ok(removed)
}
'''
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("delete packs ok")

# 3) views/mods.rs: «Удалить все модпаки» button in installed page
p = "src/views/mods.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''                        if widgets::button_sized(
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
                        }''', '''                        if widgets::button_sized(
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
                        if widgets::button_sized(
                            ui,
                            skin,
                            if confirm_packs {
                                "Точно удалить модпаки?"
                            } else {
                                "Удалить модпаки"
                            },
                            if confirm_packs { Btn::Primary } else { Btn::Ghost },
                            true,
                            26.0,
                            11.0,
                            10.5,
                        )
                        .clicked()
                        {
                            if confirm_packs {
                                delete_packs = true;
                                confirm_packs = false;
                            } else {
                                confirm_packs = true;
                            }
                        }''')
src = src.replace('''        let mut delete_all = false;
        let mut delete_group: Option<String> = None;''', '''        let mut delete_all = false;
        let mut delete_packs = false;
        let mut confirm_packs = false;
        let mut delete_group: Option<String> = None;''')
src = src.replace('''        if delete_all {''', '''        if delete_packs {
            let mut cfg = self.cfg.clone();
            match crate::profiles::delete_all_modpack_profiles(&mut cfg) {
                Ok(n) => {
                    self.cfg = cfg;
                    self.save_cfg();
                    self.toast(format!("Удалено модпаков: {n}"));
                }
                Err(e) => self.toast(format!("Не удалить: {e:#}")),
            }
        }
        if delete_all {''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("delete packs ui ok")

# 4) app.rs: auto-install client right after pack profile switch
p = "src/app.rs"
src = io.open(p, encoding="utf-8").read()
old4 = '''        if let Some((mc, loader)) = pack_meta {
            match crate::profiles::create_modpack_profile(&pack_title, &mc, &loader) {
                Ok(profile) => {
                    let mut cfg = self.cfg.clone();
                    if crate::profiles::switch_profile(&mut cfg, &profile.id).is_ok() {
                        self.cfg = cfg;
                        config::save(&self.cfg);
                        self.shared.set_cfg(self.cfg.clone());
                        self.toast(format!(
                            "Профиль «{}» · Minecraft {} · {}",
                            profile.name, profile.mc_version, profile.loader
                        ));
                    }
                }
                Err(e) => self.toast(format!("Профиль пака: {e:#}")),
            }
        }'''
new4 = '''        if let Some((mc, loader)) = pack_meta {
            match crate::profiles::create_modpack_profile(&pack_title, &mc, &loader) {
                Ok(profile) => {
                    let mut cfg = self.cfg.clone();
                    if crate::profiles::switch_profile(&mut cfg, &profile.id).is_ok() {
                        self.cfg = cfg;
                        config::save(&self.cfg);
                        self.shared.set_cfg(self.cfg.clone());
                        self.toast(format!(
                            "Профиль «{}» · Minecraft {} · {}",
                            profile.name, profile.mc_version, profile.loader
                        ));
                    }
                    // Client for the pack's version downloads immediately —
                    // «Играть» later just runs it. No duplicate downloads:
                    // install() skips everything already on disk.
                    let have = self
                        .installed()
                        .iter()
                        .any(|v| v.mc == profile.mc_version && v.loader == profile.loader);
                    if !have && profile.loader == "forge" {
                        if let Some(ctx) = self.ctx.clone() {
                            self.start_install(
                                &ctx,
                                profile.mc_version.clone(),
                                "forge".to_string(),
                                false,
                            );
                        }
                    }
                }
                Err(e) => self.toast(format!("Профиль пака: {e:#}")),
            }
        }'''
assert old4 in src, "pack_meta handler not found"
src = src.replace(old4, new4, 1)
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("auto client install ok")
