//! Everything that differs between Windows, Linux and macOS in one place:
//! names Mojang/Adoptium use for this OS, the Java binary, the classpath
//! separator and how a helper process is started without a console window.

use std::process::Command;

/// `os.name` in Mojang version rules and the natives map.
pub const MOJANG_OS: &str = if cfg!(windows) {
    "windows"
} else if cfg!(target_os = "macos") {
    "osx"
} else {
    "linux"
};

/// `os.arch` in Mojang rules (only 64-bit builds of the launcher exist).
pub const MOJANG_ARCH: &str = if cfg!(target_arch = "aarch64") { "arm64" } else { "x86_64" };

/// Joins classpath entries — `${classpath_separator}` in version files.
pub const CLASSPATH_SEP: &str = if cfg!(windows) { ";" } else { ":" };

/// File name of the Java launcher inside a JRE's `bin` folder.
pub const JAVA_BIN: &str = if cfg!(windows) { "java.exe" } else { "java" };

/// Adoptium API names for this OS and CPU.
pub const ADOPTIUM_OS: &str = if cfg!(windows) {
    "windows"
} else if cfg!(target_os = "macos") {
    "mac"
} else {
    "linux"
};
pub const ADOPTIUM_ARCH: &str = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x64" };

/// Java archives: zip on Windows, tar.gz everywhere else.
pub const JAVA_ARCHIVE_EXT: &str = if cfg!(windows) { "zip" } else { "tar.gz" };

/// Short OS name for messages shown to the user.
pub const OS_LABEL: &str = if cfg!(windows) {
    "Windows"
} else if cfg!(target_os = "macos") {
    "macOS"
} else {
    "Linux"
};

/// Settings texts that name OS places. Windows strings are unchanged.
#[cfg(windows)]
pub mod text {
    pub const DATA_LOCAL: &str = "Всё хранится локально в %APPDATA%\\Sad Launcher";
    pub const GAME_DIR_EMPTY: &str = "Пусто — %APPDATA%\\Sad Launcher\\game";
    pub const JAVA_PATH: &str = "Путь к java.exe";
    pub const JAVA_AUTO: &str = "Пусто — автопоиск: JAVA_HOME, PATH, Program Files";
    pub const AUTOSTART: &str = "Автозапуск с Windows";
}
#[cfg(target_os = "macos")]
pub mod text {
    pub const DATA_LOCAL: &str = "Всё хранится локально в ~/Library/Application Support/Sad Launcher";
    pub const GAME_DIR_EMPTY: &str = "Пусто — ~/Library/Application Support/Sad Launcher/game";
    pub const JAVA_PATH: &str = "Путь к java";
    pub const JAVA_AUTO: &str = "Пусто — автопоиск: JAVA_HOME, PATH, /Library/Java";
    pub const AUTOSTART: &str = "Автозапуск при входе в систему";
}
#[cfg(all(unix, not(target_os = "macos")))]
pub mod text {
    pub const DATA_LOCAL: &str = "Всё хранится локально в ~/.local/share/Sad Launcher";
    pub const GAME_DIR_EMPTY: &str = "Пусто — ~/.local/share/Sad Launcher/game";
    pub const JAVA_PATH: &str = "Путь к java";
    pub const JAVA_AUTO: &str = "Пусто — автопоиск: JAVA_HOME, PATH, /usr/lib/jvm";
    pub const AUTOSTART: &str = "Автозапуск при входе в систему";
}

/// `CREATE_NO_WINDOW`: a console app started from the GUI gets no window.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A command that never flashes a console window (Windows); elsewhere a
/// plain command — GUI apps have no console to show there.
pub fn hidden_command(exe: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Same as `hidden_command`, plus a priority bump for the game on Windows
/// (ABOVE_NORMAL: wins CPU time over background apps, never over the system).
pub fn game_command(exe: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW | 0x0000_8000);
    }
    cmd
}

/// `bin/java` (or `bin\java.exe`) under a JRE home.
pub fn java_in(home: &std::path::Path) -> std::path::PathBuf {
    home.join("bin").join(JAVA_BIN)
}

/// Marks a downloaded program as executable (no-op on Windows).
pub fn make_executable(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mut perm = meta.permissions();
            perm.set_mode(perm.mode() | 0o755);
            let _ = std::fs::set_permissions(path, perm);
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_this_os() {
        if cfg!(windows) {
            assert_eq!(MOJANG_OS, "windows");
            assert_eq!(CLASSPATH_SEP, ";");
            assert_eq!(JAVA_BIN, "java.exe");
            assert_eq!(JAVA_ARCHIVE_EXT, "zip");
        } else {
            assert_eq!(CLASSPATH_SEP, ":");
            assert_eq!(JAVA_BIN, "java");
            assert_eq!(JAVA_ARCHIVE_EXT, "tar.gz");
        }
        assert!(java_in(std::path::Path::new("jre")).ends_with(JAVA_BIN));
    }
}
