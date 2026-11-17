# Sad Launcher

Лаунчер Minecraft на Rust (egui). Модпаки Modrinth и CurseForge в одно нажатие, отдельная папка на каждую сборку, свой Forge-сервер и комнаты для игры с друзьями, Java ставится сама. Интерфейс на русском, чёрно-белый.

Сейчас только Windows 10/11 x64.

## Сборка

```bash
cargo build --release          # лаунчер: target/release/sad-launcher.exe
bash scripts/make-dist.sh      # лаунчер + установщик, копия установщика в site/download/
cargo test                     # тесты (живые сетевые: cargo test live_ -- --ignored --nocapture)
```

## Сайт

Статический, лежит в `site/` (HTML, CSS, JS без сборки).

- Локально: `Запустить сайт.bat` → http://localhost:8080
- Netlify: подключить репозиторий, `netlify.toml` уже указывает папку `site`
- Vercel: подключить репозиторий, `vercel.json` уже указывает папку `site`
- GitHub Pages: Settings → Pages → Source: GitHub Actions (workflow `.github/workflows/pages.yml`)

Установщик для кнопки «Скачать» лежит в `site/download/SadLauncherSetup.exe`.

## Устройство

История, архитектура и известные грабли: `RRR.md`. Протокол работы и скиллы: `SKILLS.md`, `.claude/skills/allskill`.

Не связан с Mojang и Microsoft. Minecraft является товарным знаком Mojang AB.
