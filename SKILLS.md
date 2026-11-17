# Скиллы Sad Launcher

Вызов: `/имя задача`. Протокол на каждую задачу — `/allskill`. Список обновлён 2026-11-17.
Скиллы лежат глобально в `~/.claude/skills`, команды — в `~/.claude/commands`, `/allskill` — в `.claude/skills/allskill`.

## Дизайн (лаунчер и сайт)
| Команда | Зачем |
|---|---|
| `/impeccable <cmd>` | Главный дизайн-скилл. `clarify` понятные подписи · `animate` · `polish` · `critique`/`audit` · `bolder`/`quieter` · `distill` · `harden` · `layout` · `typeset` · `delight` · `optimize` · `adapt` |
| `/ui-ux-pro-max` | Справочник UI/UX: палитры, шрифты, доступность |
| `/web-interface-guidelines` | Правила Vercel: взаимодействие, анимация, формы, скорость |
| `/taste-skill`, `/redesign-skill` | Против «шаблонного» вида; подтянуть до премиума |
| `/design-system` | Токены и спецификации компонентов |
| `/ui-audit`, `/apple-hig`, `/tov`, `/live` | «Нейрослоп» в вёрстке · Apple HIG · редактура текстов · живой аудит в браузере |

## Тексты и подача (сайт)
`/copywriting` тексты сайта · `/cro` взгляд посетителя: ценность за 5 с, CTA · `/marketing-psychology` — только честные приёмы.
`/pricing`, `/signup` — лаунчеру не нужны (бесплатный, без регистрации).

## Видео (Remotion)
`/remotion-best-practices` (маршрутизатор) · `/remotion-create` · `/remotion-markup` · `/remotion-render` · `/remotion-studio`.

## Разработка
| Команда | Зачем |
|---|---|
| `/investigate-first` | Неясный сбой или медленно — сначала причина и замер |
| `/surgical-patch` | Баг — правка в самом узком слое + тест |
| `/lean-build` | Новая функция без лишнего |
| `/safe-refactor` | Рефакторинг без смены поведения |
| `/migration` | Обратимые миграции конфига/данных |
| `/verify-and-stop` | Доказать, что готово |
| `/ecc-plan`, `/ecc-tdd`, `/ecc-code-review`, `/ecc-verify`, `/ecc-build-fix` | План · тесты сначала · ревью · проверка · починка сборки |
| `/caveman`, `/caveman-review`, `/caveman-explore`, `cavecrew` | Короткие ответы · ревью строкой · поиск по коду без раздувания контекста |
| `ecc-security-review` | Архивы (zip-slip), пути, сеть, ввод |

**Не использовать:** `caveman-setup`, `-discover`, `-evidence-review`, `-manage`, `-optimize` (Caveman Cloud, сторонний шлюз), `megacave`,
`roblox-*`, `screenpipe-*`, `/tg-sync` (для лаунчера не нужны).

## MCP
`context7` (документация крейтов), браузер приложения (сайт), Claude in Chrome.
