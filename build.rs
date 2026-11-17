//! Embeds the launcher icon into the exe so shortcuts pick it up.
//! windres (GNU) chokes on spaces in paths, so the .rc is built in %TEMP%.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_WINDOWS").is_err() {
        return;
    }

    let Some(tmp) = std::env::var_os("TEMP").map(PathBuf::from) else {
        return;
    };
    let work = tmp.join("madrid-launcher-icon");
    if std::fs::create_dir_all(&work).is_err() {
        return;
    }
    if std::fs::copy("assets/icon.ico", work.join("icon.ico")).is_err() {
        return;
    }
    if std::fs::write(work.join("icon.rc"), "1 ICON \"icon.ico\"\n").is_err() {
        return;
    }

    let out = work.join("icon.o");
    let ok = Command::new("windres")
        .args([
            "-O",
            "coff",
            "-o",
            out.to_string_lossy().as_ref(),
            "icon.rc",
        ])
        .current_dir(&work)
        .output();
    match ok {
        Ok(o) if o.status.success() => {
            println!("cargo:rustc-link-arg={}", out.display());
        }
        _ => {
            // Icon is cosmetic; the build must not fail without windres.
        }
    }
}
