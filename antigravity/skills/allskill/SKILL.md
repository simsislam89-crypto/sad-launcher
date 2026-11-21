---
name: allskill
description: Обязательный протокол для каждой задачи проекта Sad Launcher (Rust-лаунчер Minecraft + сайт) — подбор и ВЫЗОВ скиллов из SKILLS.md, правила Rust-only, проверка cargo/скриншотами/браузером перед словом «готово». Вызов — /allskill <задача>, но протокол действует без вызова, в каждом ответе.
---

# allskill — обязательный протокол Sad Launcher

Каждый пункт — действие, а не справка. Порядок: шаг 0 → 1 → 2 → 3 → 4 → 5.

## Шаг 0. Контекст проекта — до первой правки

1. Прочитай `RRR.md` в корне проекта (история, грабли, архитектура). Он — источник правды о коде.
2. Код лаунчера — ТОЛЬКО Rust (egui/eframe). Новые функции, утилиты, патчи — на Rust.
   Исключение: сайт `site/` (HTML/CSS/JS) и батники запуска.
3. Весь интерфейс и ответы — на русском. Дизайн — чёрно-белый, прозрачные (стеклянные) кнопки без сплошной заливки; допустимы только зелёный (одна сеть/готово) и красный (ошибки).
4. Первая строка ответа — суть. Без вступлений. Сжатие ответов — по `ultracave.md` этой папки, если владелец просит коротко.

## Шаг 0.5. Нет скилла — скачай все сразу

1. Перед шагом 2 сверь нужные скиллы со списком доступных (Skill). Нет хоть одного — сразу выполни установку ниже,
   не спрашивая владельца, затем продолжай задачу. Ставятся только отсутствующие скиллы, свои правки не затираются.
2. Пакет скиллов и команд (`/tov`, `/ui-audit`, `/live`, `/apple-hig`, `ecc-*`…) лежит в публичном репозитории
   `simsislam89-crypto/sad-launcher`, папки `antigravity/skills` и `antigravity/commands`. Новый скилл — положить туда же.

Git Bash / macOS / Linux:
```bash
D="${TMPDIR:-/tmp}/sad-skills"; rm -rf "$D"
git clone -q --depth 1 --filter=blob:none --sparse https://github.com/simsislam89-crypto/sad-launcher "$D"
git -C "$D" sparse-checkout set antigravity/skills antigravity/commands
mkdir -p ~/.claude/skills ~/.claude/commands
for s in "$D"/antigravity/skills/*/; do n=$(basename "$s"); [ -d ~/.claude/skills/"$n" ] || cp -r "$s" ~/.claude/skills/; done
cp -n "$D"/antigravity/commands/*.md ~/.claude/commands/ 2>/dev/null; rm -rf "$D"
```

PowerShell:
```powershell
$d = "$env:TEMP\sad-skills"; Remove-Item $d -Recurse -Force -ErrorAction SilentlyContinue
git clone -q --depth 1 --filter=blob:none --sparse https://github.com/simsislam89-crypto/sad-launcher $d
git -C $d sparse-checkout set antigravity/skills antigravity/commands
$sk = "$HOME\.claude\skills"; $cm = "$HOME\.claude\commands"; New-Item -ItemType Directory -Force $sk, $cm | Out-Null
Get-ChildItem "$d\antigravity\skills" -Directory | Where-Object { -not (Test-Path "$sk\$($_.Name)") } | Copy-Item -Destination $sk -Recurse
Get-ChildItem "$d\antigravity\commands\*.md" | Where-Object { -not (Test-Path "$cm\$($_.Name)") } | Copy-Item -Destination $cm
Remove-Item $d -Recurse -Force
```

Не из пакета — отдельными командами:
- Плагин superpowers: `claude plugin install superpowers@claude-plugins-official`.
- CLI для `rust-anything`: `cargo install rust-anything --locked`, затем `rust-anything setup`.
- Первоисточники (обновить скилл до свежей версии): impeccable — github.com/pbakaus/impeccable, caveman — github.com/JuliusBrussee/caveman,
  rust-anything — github.com/erkamyaman/rust-anything, маркетинг (cro, signup, pricing) — github.com/coreyhaines31/marketingskills,
  web-interface-guidelines — github.com/vercel-labs/web-interface-guidelines, remotion-* — github.com/remotion-dev/remotion.

## Шаг 1. Прочитай SKILLS.md

1. Открой `SKILLS.md` в корне проекта целиком.
2. Запиши одной строкой: «Задача: <тип>. Скиллы: <список>».

## Шаг 2. Обязательный минимум скиллов по типу задачи

- **Интерфейс лаунчера (egui)**: `impeccable` (режимы `clarify` — понятные подписи, `animate` — анимации,
  `polish`, `harden`) + хотя бы один из `ui-ux-pro-max`, `web-interface-guidelines`, `taste-skill`.
  Любой текст интерфейса — через `/tov` (короткие кнопки с глаголом: «Добавить», «Удалить», «Установить»).
  Иконка без подписи у главного действия — ошибка: рядом нужна надпись.
- **Сайт лаунчера (`site/`)**: не меньше двух из `impeccable`, `taste-skill`, `redesign-skill`, `ui-ux-pro-max`,
  `web-interface-guidelines`, `design-system`. Тексты — `copywriting` + `/tov`. После правок — `/ui-audit` и `/live`.
  Идеи компонентов — 21st.dev и reactbits.dev (перенести в чистый HTML/CSS/JS, без сборщиков).
- **Скорость (загрузка, распаковка, удаление, FPS, память)**: `investigate-first` — сначала замер и причина,
  потом правка. Цифры «до/после» обязательны.
- **Новая функция**: `lean-build`. Больше одного файла — сначала `ecc-plan`.
- **Баг**: `investigate-first` → `surgical-patch`.
- **Рефакторинг**: `safe-refactor`. **Миграция конфига/данных**: `migration` (serde(default), обратимо).
- **Ввод пользователя, сеть, файлы с диска/из архивов**: `ecc-security-review` (zip-slip, пути, размеры).
- **Тесты**: `ecc-tdd` для новой чистой логики (тесты внутри модулей `#[cfg(test)]`).
- **Поиск по коду**: `caveman-explore` или `cavecrew`.
- **Ревью**: `caveman-review` или `ecc-code-review`.
- **Видео/промо-ролик**: `remotion-best-practices` + `remotion-create` / `remotion-render`.
- **Перенос кода на Rust (Python/JS/TS/C/Go/Ruby/Java/скрипты → Rust)**: `rust-anything` — по одному блоку:
  замер (`profile`) → план (`analyze`, ждать одобрения) → `bridge` → `record` поведения оригинала → порт в `port.rs`
  → `diff` N/N → `bench` → `gate`. Готово — только когда `gate` прошёл. Целиком приложение — по `whole-app.md` скилла.
  CLI: `cargo install rust-anything` + `rust-anything setup` (если нет в PATH — попросить владельца поставить).
  Скилл: `~/.claude/skills/rust-anything` (источник github.com/erkamyaman/rust-anything).

## Шаг 3. Вызов скиллов — через Skill, каждый отдельно

1. Каждый скилл из шага 2 — отдельный вызов Skill. Не один «главный» вместо всех.
2. Скилла нет — одна строка пользователю, работа продолжается.
3. `/ui-audit`, `/live`, `/tov`, `/apple-hig` — команды; запускать как команды.

## Шаг 4. MCP и инструменты

- `context7` — документация egui/eframe/tokio/reqwest/zip перед использованием незнакомого API.
- Браузер приложения — проверка сайта (375 px и 1440 px, консоль без ошибок).
- Окно лаунчера: `MADRID_DEV_TAB=play|versions|build|mods|modpacks|server|settings` +
  `MADRID_SECOND_INSTANCE=1` — открыть нужную вкладку рядом с запущенной копией и снять скриншот.
- НЕ делать скриншоты с взаимодействием, если владелец играет в fullscreen (проверять логи/процессы).

## Шаг 5. Проверка — перед словом «готово»

1. `cargo check` без ошибок и новых предупреждений, затем `cargo test` (живые тесты — `cargo test live_ -- --ignored`
   только по задаче).
2. Сборка дистрибутива: `bash scripts/make-dist.sh`.
3. UI: скриншот затронутой вкладки; наложения, обрезка текста, анимация.
4. Сайт: открыть `site/` через батник, проверить консоль и мобильную ширину.
5. **Всегда** обновить `RRR.md` разделом «Обновление <дата>» — что изменено и какие грабли найдены.
6. Вызвать `verify-and-stop`. Без этого не писать «готово», «работает», «исправлено».
7. **Всегда** после проверок сделать `git commit` и `git push` в `main` сразу, без вопроса (без секретов).
8. **Выпуск версии** — если менялся код лаунчера (`src/`), агент САМ выпускает новую версию, не спрашивая: поднять patch в `Cargo.toml` (0.7.1 → 0.7.2), `bash scripts/make-dist.sh` (пишет `site/version.json` и установщик), commit + push в `main` — у пользователей при запуске появится «Обновиться». Только сайт/документы — версию не поднимать.

## Запрещено

- `caveman-setup`, `caveman-discover`, `caveman-evidence-review`, `caveman-manage`, `caveman-optimize` — Caveman Cloud,
  гонит запросы через сторонний шлюз.
- `megacave` — китайская проза, для русского непригоден.
- Код лаунчера не на Rust (Python-патчи — только одноразовые скрипты в scratchpad, не в проекте).
- Запуск/подключение чего-либо без явного действия пользователя; консольные окна (`CREATE_NO_WINDOW`).
- Секреты (`.env`, токены, ключи CF) в коммитах или логах.
