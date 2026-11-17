# AGENTS.md — Sad Launcher

Rust/egui-лаунчер Minecraft + статический сайт `site/`. Рабочая папка `D:\working WIN\LLM launcher`, Windows 11, Git Bash.

Порядок входа для любого агента (Antigravity, Claude Code, Codex):

1. Прочитать `ANTIGRAVITY.md` (установка скиллов и полный контекст), затем `RRR.md` (история и грабли) и `SKILLS.md` (каталог скиллов).
2. Включить протокол `allskill` (`.agent/skills/allskill/SKILL.md` или `.claude/skills/allskill/SKILL.md`): он действует в каждом ответе.
3. Код лаунчера — только Rust. Интерфейс и ответы — на русском. Тема — чёрно-белая, стеклянная с прозрачными кнопками без сплошной заливки; допустимы только зелёный (одна сеть/готово) и красный (ошибки).
4. Перед словом «готово»: `cargo check`, `cargo test`, `bash scripts/make-dist.sh`, скриншот/браузер, **всегда обновить `RRR.md`**, прогнать `verify-and-stop` и **всегда сделать `git commit` и `git push` в `main`**.
5. Секреты (`.env`, токены, ключи) не читать и не коммитить. Папку проекта и `%APPDATA%\Sad Launcher` не удалять.
6. Запускать собранный лаунчер только с `MADRID_SECOND_INSTANCE=1`.
