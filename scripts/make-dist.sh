#!/usr/bin/env bash
# Сборка дистрибутива теперь на Rust: xtask/src/main.rs.
# Лаунчер + установщик в корень проекта, site/download и site/version.json.
set -e
cd "$(dirname "$0")/.."
exec cargo run --quiet -p xtask -- dist
