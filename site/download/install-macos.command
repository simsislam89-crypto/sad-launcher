#!/usr/bin/env bash
set -euo pipefail

echo "== Автоустановка Sad Launcher для macOS =="

SITE_URL="https://rkt5jy.bot-keep.xyz/download/SadLauncher-macos-universal.tar.gz"
GH_URL="https://github.com/simsislam89-crypto/sad-launcher/releases/latest/download/SadLauncher-macos-universal.tar.gz"
TARGET_APP="/Applications/Sad Launcher.app"
[ -w "/Applications" ] || TARGET_APP="${HOME}/Applications/Sad Launcher.app"
mkdir -p "$(dirname "$TARGET_APP")"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

installed=0
for url in "$SITE_URL" "$GH_URL"; do
  echo "Проверяю готовое приложение: $url"
  if curl -fsSL "$url" -o "$TMP/app.tar.gz" 2>/dev/null; then
    tar -xzf "$TMP/app.tar.gz" -C "$TMP"
    if [ -d "$TMP/Sad Launcher.app" ]; then
      rm -rf "$TARGET_APP"
      mv "$TMP/Sad Launcher.app" "$TARGET_APP"
      xattr -dr com.apple.quarantine "$TARGET_APP" >/dev/null 2>&1 || true
      installed=1
      break
    fi
  fi
done

if [ "$installed" -eq 0 ]; then
  echo "Готовый пакет ещё не загружен — выполняю автоматическую сборку и упаковку .app..."
  if ! xcode-select -p >/dev/null 2>&1; then
    echo "Нужны Xcode Command Line Tools — запускаю установку..."
    xcode-select --install || true
    echo "После завершения установки Xcode Tools запусти этот файл повторно."
    exit 1
  fi
  if ! command -v cargo >/dev/null 2>&1; then
    echo "Устанавливаю Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck disable=SC1091
    source "${HOME}/.cargo/env"
  fi
  git clone --depth 1 "https://github.com/simsislam89-crypto/sad-launcher.git" "$TMP/src"
  cargo build --release --manifest-path "$TMP/src/Cargo.toml" --bin sad-launcher

  rm -rf "$TARGET_APP"
  mkdir -p "$TARGET_APP/Contents/MacOS" "$TARGET_APP/Contents/Resources"
  cp -f "$TMP/src/target/release/sad-launcher" "$TARGET_APP/Contents/MacOS/sad-launcher"
  chmod +x "$TARGET_APP/Contents/MacOS/sad-launcher"
  [ -f "$TMP/src/assets/icon.png" ] && cp -f "$TMP/src/assets/icon.png" "$TARGET_APP/Contents/Resources/icon.png"
  cat > "$TARGET_APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>sad-launcher</string>
  <key>CFBundleIdentifier</key><string>ru.sadlauncher.app</string>
  <key>CFBundleName</key><string>Sad Launcher</string>
  <key>CFBundleDisplayName</key><string>Sad Launcher</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.7.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
fi

echo "Готово! Установлено в: $TARGET_APP"
open "$TARGET_APP"
