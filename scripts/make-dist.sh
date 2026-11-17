#!/usr/bin/env bash
# Builds the launcher + a self-contained installer and drops both
# into the project root where they are easy to find.
set -e
cd "$(dirname "$0")/.."

echo "== сборка лаунчера =="
cargo build --release --bin sad-launcher

echo "== кложу payload для установщика =="
cp -f target/release/sad-launcher.exe assets/launcher-payload.exe
if ! cp -f target/release/sad-launcher.exe "Sad Launcher.exe" 2>/dev/null; then
  cp -f target/release/sad-launcher.exe "Sad Launcher.new.exe"
  echo "Внимание: Sad Launcher.exe занят запущенным процессом — новая сборка сохранена в Sad Launcher.new.exe"
fi

echo "== сборка установщика =="
cargo build --release --bin installer

if ! cp -f target/release/installer.exe "Установить Sad Launcher.exe" 2>/dev/null; then
  cp -f target/release/installer.exe "Установить Sad Launcher.new.exe"
fi
mkdir -p site/download && cp -f target/release/installer.exe site/download/SadLauncherSetup.exe

VER=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
SIZE=$(wc -c < site/download/SadLauncherSetup.exe | tr -d ' ')
cat > site/version.json <<EOF
{
  "version": "${VER}",
  "size": ${SIZE},
  "url": "https://rkt5jy.bot-keep.xyz/download/SadLauncherSetup.exe"
}
EOF

echo "== готово =="
ls -la *.exe
