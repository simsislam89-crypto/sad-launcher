---
name: allskill
description: Обязательный протокол каждой задачи Sad Launcher (Rust/egui-лаунчер Minecraft + сайт site/) — контекст из RRR.md, автоустановка недостающих скиллов, подбор и вызов скиллов, агентов и MCP по типу задачи, проверка и выпуск версии. Вызов — /allskill <задача>; действует в каждом ответе и без вызова.
---

# allskill — протокол Sad Launcher

Каждый пункт — действие. Порядок: 0 → 1 → 2 → 3 → 4 → 5.

## 0. Контекст и правила

1. Прочитай `RRR.md` (история, архитектура, грабли) — источник правды о коде.
2. Код лаунчера — только Rust (egui/eframe). Исключения: сайт `site/` (HTML/CSS/JS), ролики `video/` (Remotion), батники.
3. Интерфейс и ответы — на русском. Тема чёрно-белая, стеклянные кнопки без заливки; цвета — только зелёный (готово/одна сеть) и красный (ошибка).
4. Первая строка ответа — суть, без вступлений.

## 1. Скиллы на месте?

Сверь нужные скиллы (шаг 2) со списком Skill. Нет хоть одного — сразу, без вопроса, поставь недостающие и продолжай.
Ставятся только отсутствующие; свои правки не затираются. Пакет: публичный `simsislam89-crypto/sad-launcher`,
папки `antigravity/skills` (49 скиллов) и `antigravity/commands` (20 команд). Новый скилл — класть туда же.

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

Отдельно: плагин `claude plugin install superpowers@claude-plugins-official`; CLI `cargo install rust-anything --locked` → `rust-anything setup`.

## 2. Скиллы по типу задачи

Запиши одной строкой: «Задача: <тип>. Скиллы: <список>». Минимум:

| Задача | Скиллы |
|---|---|
| Интерфейс лаунчера (egui) | `impeccable` (`clarify`, `animate`, `polish`, `harden`, `layout`) + один из `ui-ux-pro-max`, `web-interface-guidelines`, `taste-skill`; тексты — `/tov`; проверка — `/apple-hig` |
| Сайт `site/` | два из `impeccable`, `taste-skill`, `redesign-skill`, `ui-ux-pro-max`, `web-interface-guidelines`, `design-system`; тексты `copywriting` + `/tov`; подача `cro`, `marketing-psychology` (только честные приёмы); после правок `/ui-audit`, `/live` |
| Скорость (загрузка, распаковка, FPS, память) | `investigate-first`: замер и причина до правки, цифры «до/после» |
| Баг | `investigate-first` → `surgical-patch` (или `superpowers:systematic-debugging`) |
| Новая функция | `lean-build`; больше одного файла — `ecc-plan` |
| Рефакторинг / чистка | `safe-refactor`, `ecc-refactor-clean` |
| Миграция конфига/данных | `migration` (serde(default), обратимо) |
| Сеть, архивы, ввод, файлы | `ecc-security-review` (zip-slip, пути, размеры, https) |
| Тесты | `ecc-tdd` / `ecc-tdd-workflow` (`#[cfg(test)]` в модуле), `ecc-test-coverage` |
| Перенос кода на Rust | `rust-anything`: `profile` → `analyze` (ждать одобрения) → `bridge` → `record` → порт → `diff` N/N → `bench` → `gate` |
| Поиск по коду | `caveman-explore`, `cavecrew` |
| Ревью | `caveman-review`, `ecc-code-review`, `code-review` |
| Сборка упала | `ecc-build-fix` |
| Документация | `ecc-update-docs`, `ecc-update-codemaps` |
| Ролик / промо | `remotion-best-practices` + `remotion-create`, `remotion-markup`, `remotion-render`, `remotion-studio`, `remotion-docs` |
| Короткие ответы / коммит | `caveman`, `ultracave`, `caveman-commit` |
| Проверка готовности | `verify-and-stop` (всегда в конце), `ecc-verify`, `ecc-verification-loop` |

Каждый скилл — отдельный вызов Skill. `/tov`, `/ui-audit`, `/live`, `/apple-hig` — команды.

## 3. Агенты

Агента — только когда задача правда выигрывает от отдельного контекста:
- `Explore` / `cavecrew-investigator` — широкий поиск по многим файлам;
- `ecc-planner`, `Plan`, `ecc-architect` — план большой фичи или архитектуры;
- `ecc-code-reviewer`, `ecc-security-reviewer` — ревью после крупной правки;
- `ecc-build-error-resolver` — красная сборка;
- `ecc-tdd-guide`, `ecc-e2e-runner` — тесты;
- `ecc-refactor-cleaner`, `ecc-doc-updater` — чистка мёртвого кода, документация;
- `web-fetch` — прочитать страницу/доку по ссылке.

## 4. MCP и инструменты

- `context7` — документация egui/eframe/tokio/reqwest/zip перед незнакомым API.
- Браузер приложения — сайт на 375 и 1440 px, консоль без ошибок.
- Окно лаунчера: `MADRID_SECOND_INSTANCE=1` + `MADRID_DEV_TAB=play|versions|build|mods|modpacks|server|settings`,
  `MADRID_DEV_QUERY=…`, `MADRID_DEV_DETAIL=1` (карточка первого результата), `MADRID_DEV_SHOT=<png>` `MADRID_DEV_SHOT_DELAY=<с>`.
- Не делать скриншоты с взаимодействием, если владелец играет в fullscreen.

## 5. Перед словом «готово»

1. `cargo check` без ошибок и новых предупреждений, `cargo test` (живые `cargo test live_ -- --ignored` — по задаче).
2. `bash scripts/make-dist.sh` (Rust-крейт `xtask`).
3. Скриншот затронутой вкладки; сайт — браузер, мобильная ширина.
4. Дописать в `RRR.md` раздел «Обновление <дата>»: что сделано, грабли.
5. `verify-and-stop`.
6. Менялся `src/` — поднять patch в `Cargo.toml`, снова `make-dist.sh` (пишет `site/version.json` и установщик).
7. `git commit` + `git push` в `main` сразу, без вопроса.
8. Менялся `site/` — синхронизировать с `erootstars/launcher_site` по AGENTS.md п. 8.

## Запрещено

- `caveman-setup`, `-discover`, `-evidence-review`, `-manage`, `-optimize` (сторонний шлюз), `megacave`, `roblox-*`, `screenpipe-*`, `/tg-sync`.
- Код лаунчера не на Rust; Python — только разовые скрипты в scratchpad.
- Запуск или подключение чего-либо без действия пользователя; консольные окна (`CREATE_NO_WINDOW`).
- Секреты (`.env`, токены, ключи) в коммитах и логах; удаление папки проекта и `%APPDATA%\Sad Launcher`.
