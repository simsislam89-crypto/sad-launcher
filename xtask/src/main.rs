//! Сборка дистрибутива Sad Launcher на Rust (раньше — bash `make-dist.sh`).
//!
//! `cargo run -p xtask -- dist` (или `bash scripts/make-dist.sh`, который
//! теперь просто вызывает эту команду):
//! 1. собирает лаунчер (release) и кладёт его в `assets/launcher-payload.exe`
//!    — установщик встраивает его через `include_bytes!`;
//! 2. копирует лаунчер в корень как `Sad Launcher.exe` (если файл занят
//!    запущенным лаунчером — `Sad Launcher.new.exe`);
//! 3. собирает установщик → `Установить Sad Launcher.exe` и
//!    `site/download/SadLauncherSetup.exe`;
//! 4. пишет `site/version.json` (версия из Cargo.toml + размер установщика)
//!    — по нему лаунчеры у пользователей предлагают «Обновиться».

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const SITE_URL: &str = "https://rkt5jy.bot-keep.xyz";

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "dist" => dist(),
        _ => Err("использование: cargo run -p xtask -- dist".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Ошибка: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Корень проекта: папка над `xtask/`.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn cargo_build(root: &Path, bin: &str) -> Result<(), String> {
    println!("== сборка {bin} ==");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(root)
        .args(["build", "--release", "--bin", bin])
        .status()
        .map_err(|e| format!("cargo не запустился: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("сборка {bin} завершилась ошибкой ({status})"))
    }
}

/// Копирует `from` в `to`; если `to` занят (запущен), — в `fallback`.
fn copy_or_fallback(from: &Path, to: &Path, fallback: &Path) -> Result<(), String> {
    match std::fs::copy(from, to) {
        Ok(_) => Ok(()),
        Err(_) => {
            std::fs::copy(from, fallback).map_err(|e| format!("{}: {e}", fallback.display()))?;
            println!(
                "Внимание: {} занят запущенным процессом — новая сборка сохранена в {}",
                name(to),
                name(fallback)
            );
            Ok(())
        }
    }
}

fn name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `version = "0.7.2"` из секции [package].
fn package_version(manifest: &str) -> Option<String> {
    manifest
        .lines()
        .take_while(|l| !l.trim_start().starts_with("[workspace]") && !l.trim_start().starts_with("[profile"))
        .find_map(|l| {
            let rest = l.trim().strip_prefix("version")?.trim_start().strip_prefix('=')?;
            Some(rest.trim().trim_matches('"').to_string())
        })
}

fn version_json(version: &str, size: u64) -> String {
    format!(
        "{{\n  \"version\": \"{version}\",\n  \"size\": {size},\n  \"url\": \"{SITE_URL}/download/SadLauncherSetup.exe\"\n}}\n"
    )
}

fn dist() -> Result<(), String> {
    let root = root();
    let exe = std::env::consts::EXE_SUFFIX;
    let release = root.join("target").join("release");

    cargo_build(&root, "sad-launcher")?;
    let launcher = release.join(format!("sad-launcher{exe}"));
    println!("== кладу payload для установщика ==");
    std::fs::copy(&launcher, root.join("assets").join("launcher-payload.exe"))
        .map_err(|e| format!("payload: {e}"))?;
    copy_or_fallback(&launcher, &root.join("Sad Launcher.exe"), &root.join("Sad Launcher.new.exe"))?;

    cargo_build(&root, "installer")?;
    let installer = release.join(format!("installer{exe}"));
    copy_or_fallback(
        &installer,
        &root.join("Установить Sad Launcher.exe"),
        &root.join("Установить Sad Launcher.new.exe"),
    )?;
    let download = root.join("site").join("download");
    std::fs::create_dir_all(&download).map_err(|e| format!("site/download: {e}"))?;
    let setup = download.join("SadLauncherSetup.exe");
    std::fs::copy(&installer, &setup).map_err(|e| format!("{}: {e}", setup.display()))?;

    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).map_err(|e| format!("Cargo.toml: {e}"))?;
    let version = package_version(&manifest).ok_or("в Cargo.toml нет version")?;
    let size = std::fs::metadata(&setup).map_err(|e| e.to_string())?.len();
    std::fs::write(root.join("site").join("version.json"), version_json(&version, size))
        .map_err(|e| format!("site/version.json: {e}"))?;

    println!("== готово: версия {version} ==");
    for entry in std::fs::read_dir(&root).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("exe") {
            let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
            println!("{:>12}  {}", len, name(&path));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_package_version_not_dependency_versions() {
        let manifest = "[package]\nname = \"sad-launcher\"\nversion = \"0.7.3\"\n\n[workspace]\nmembers = [\"xtask\"]\n\n[dependencies]\ntokio = { version = \"1\" }\n";
        assert_eq!(package_version(manifest).as_deref(), Some("0.7.3"));
    }

    #[test]
    fn version_json_matches_the_launcher_format() {
        let json = version_json("0.7.3", 123);
        assert!(json.contains("\"version\": \"0.7.3\""));
        assert!(json.contains("\"size\": 123"));
        assert!(json.contains("/download/SadLauncherSetup.exe"));
    }
}
