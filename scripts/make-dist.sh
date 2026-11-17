#!/usr/bin/env bash
# Builds the launcher + a self-contained installer and drops both
# into the project root where they are easy to find.
set -e
cd "$(dirname "$0")/.."

echo "== сборка лаунчера =="
cargo build --release --bin sad-launcher

echo "== кложу payload для установщика =="
cp target/release/sad-launcher.exe assets/launcher-payload.exe
cp target/release/sad-launcher.exe "Sad Launcher.exe"

echo "== сборка установщика =="
cargo build --release --bin installer

cp -f target/release/installer.exe "Установить Sad Launcher.exe"
mkdir -p site/download && cp -f target/release/installer.exe site/download/SadLauncherSetup.exe

echo "== готово =="
ls -la *.exe
