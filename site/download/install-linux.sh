#!/usr/bin/env bash
set -euo pipefail

echo "== Автоустановка Sad Launcher для Linux =="

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64) PKG="sad-launcher-linux-x86_64.tar.gz" ;;
  aarch64|arm64) PKG="sad-launcher-linux-aarch64.tar.gz" ;;
  *) PKG="sad-launcher-linux-${ARCH}.tar.gz" ;;
esac

SITE_BASE="https://rkt5jy.bot-keep.xyz/download"
GH_BASE="https://github.com/simsislam89-crypto/sad-launcher/releases/latest/download"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/256x256/apps"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR"

installed=0
for url in "${GH_BASE}/${PKG}" "${SITE_BASE}/${PKG}"; do
  echo "Проверяю готовый пакет: $url"
  if curl -fsSL "$url" -o "$TMP/pkg.tar.gz" 2>/dev/null; then
    tar -xzf "$TMP/pkg.tar.gz" -C "$TMP"
    if [ -f "$TMP/sad-launcher" ]; then
      install -m 755 "$TMP/sad-launcher" "$BIN_DIR/sad-launcher"
      [ -f "$TMP/sad-launcher.png" ] && cp -f "$TMP/sad-launcher.png" "$ICON_DIR/sad-launcher.png"
      installed=1
      break
    fi
  fi
done

if [ "$installed" -eq 0 ]; then
  echo "Готовый архив не найден на сервере — выполняю автоматическую сборку из исходников..."
  if ! command -v cargo >/dev/null 2>&1; then
    echo "Устанавливаю Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck disable=SC1091
    source "${HOME}/.cargo/env"
  fi
  git clone --depth 1 "https://github.com/simsislam89-crypto/sad-launcher.git" "$TMP/src"
  cargo build --release --manifest-path "$TMP/src/Cargo.toml" --bin sad-launcher
  install -m 755 "$TMP/src/target/release/sad-launcher" "$BIN_DIR/sad-launcher"
  [ -f "$TMP/src/assets/icon.png" ] && cp -f "$TMP/src/assets/icon.png" "$ICON_DIR/sad-launcher.png"
fi

cat > "$APP_DIR/sad-launcher.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Sad Launcher
Comment=Лаунчер Minecraft на Rust
Exec=${BIN_DIR}/sad-launcher
Icon=sad-launcher
Terminal=false
Categories=Game;
DESKTOP

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true

# The binary loads GTK (file dialogs) and OpenGL from the system.
missing="$(ldd "$BIN_DIR/sad-launcher" 2>/dev/null | awk '/not found/ {print $1}' | tr '
' ' ')"
if [ -n "$missing" ]; then
  echo "Не хватает системных библиотек: $missing"
  if command -v apt-get >/dev/null 2>&1; then echo "  sudo apt-get install -y libgtk-3-0 libgl1 libxkbcommon0"
  elif command -v dnf >/dev/null 2>&1; then echo "  sudo dnf install -y gtk3 mesa-libGL libxkbcommon"
  elif command -v pacman >/dev/null 2>&1; then echo "  sudo pacman -S --needed gtk3 mesa libxkbcommon"
  fi
fi
case ":$PATH:" in *":$BIN_DIR:"*) ;; *) echo "Совет: добавьте $BIN_DIR в PATH, чтобы запускать командой sad-launcher." ;; esac

echo "Готово! Sad Launcher есть в меню приложений. Запускаю..."
nohup "$BIN_DIR/sad-launcher" >/dev/null 2>&1 &
