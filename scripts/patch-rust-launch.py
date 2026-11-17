import io

p = "src/rustmc.rs"
src = io.open(p, encoding="utf-8").read()

# source patches after clone
src = src.replace('''/// Stevenarella closes on a stale vsync check right after boot; keep it open.
fn patch_stevenarella(repo: &PathBuf) {
    let main_rs = repo.join("src").join("main.rs");
    let Ok(text) = std::fs::read_to_string(&main_rs) else {
        return;
    };
    let broken = "error!(\\"Changing vsync currently requires restarting\\");\\n        game.should_close = true;";
    let fixed = "error!(\\"Changing vsync currently requires restarting\\");\\n        *vsync = vsync_changed;";
    if text.contains(broken) {
        let patched = text.replace(&broken, &fixed);
        let _ = std::fs::write(&main_rs, patched);
    }
}''', '''/// Source fixes applied before `cargo build`: the stale vsync close and a
/// Compatibility GL profile (Core profile segfaults 0xC0000005 on some
/// driver/overlay combos).
fn patch_stevenarella(repo: &PathBuf) {
    let main_rs = repo.join("src").join("main.rs");
    let Ok(text) = std::fs::read_to_string(&main_rs) else {
        return;
    };
    let mut out = text;
    out = out.replace(
        "error!(\\"Changing vsync currently requires restarting\\");\\n        game.should_close = true;",
        "error!(\\"Changing vsync currently requires restarting\\");\\n        *vsync = vsync_changed;",
    );
    out = out.replace("opengl_version: (3, 2),", "opengl_version: (2, 1),");
    out = out.replace("opengles_version: (3, 0),", "opengles_version: (2, 0),");
    out = out.replace(
        ".with_gl_profile(glutin::GlProfile::Core)",
        ".with_gl_profile(glutin::GlProfile::Compatibility)",
    );
    if out != text {
        let _ = std::fs::write(&main_rs, out);
    }
}''')

# watcher: reuse the same pattern as launch.rs — try_wait loop + crash hint
src = src.replace('''    shared.set_game(Some(child));
    shared.game_running.store(true, Ordering::SeqCst);
    Ok(())
}''', '''    shared.set_game(Some(child));
    shared.game_running.store(true, Ordering::SeqCst);

    // Watcher: when the GL context segfaults (0xC0000005), explain it plainly.
    {
        let shared = shared.clone();
        std::thread::Builder::new()
            .name("rustmc-watch".into())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_millis(400));
                let mut guard = match shared.game.lock() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                let Some(c) = guard.as_mut() else {
                    break;
                };
                match c.try_wait() {
                    Ok(Some(status)) => {
                        drop(guard);
                        shared.game_running.store(false, Ordering::SeqCst);
                        shared.push_game(format!(
                            "[Madrid] Rust-издание завершилось · {status}"
                        ));
                        if let Ok(code) = status.code().map(|c| c).ok_or(()).map(|c| c) {
                            if code == 0xC0000005u32 as i32 {
                                shared.push_game(
                                    "[Madrid] совет: краш видеодрайвера — отключи оверлей GeForce/Discord/RTSS или обнови драйвер",
                                );
                            }
                        }
                        if let Ok(mut g) = shared.game.lock() {
                            *g = None;
                        }
                        break;
                    }
                    Ok(None) => {}
                    Err(_) => break,
                }
            })
            .ok();
    }
    Ok(())
}''')

# Duration import
if "use std::time::Duration;" not in src:
    src = src.replace("use std::io::BufRead;", "use std::io::BufRead;\nuse std::time::Duration;")

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("watcher ok")
