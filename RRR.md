# RRR.md — полная история проекта Sad Launcher (бывш. LLM Launcher Madrid)

> Файл для мгновенного входа нейросети в контекст. Обновлён: 2026-11-21, версия лаунчера 0.7.9. Протокол задач: /allskill (.claude/skills/allskill), список скиллов: SKILLS.md.
> Язык проекта — ТОЛЬКО Rust (требование владельца).
> Проект: `D:\working WIN\LLM launcher` (Windows 11, Git Bash, Rust toolchain x86_64-pc-windows-gnu).

## Что это

Minecraft-лаунчер «LLM Launcher Madrid» на Rust (egui/eframe 0.37, glow, прозрачное безрамочное окно, чёрно-белый дизайн, весь UI на русском). Ставит Minecraft (ваниль + Forge-пакет «Мадрид»), моды/шейдеры/ресурспаки/карты/модпаки, хостит собственный локальный Forge-сервер, умеет играть с друзьями по коду, качает Java сама. Никаких внешних серверов с нашей стороны — только Mojang/Forge/Modrinth/Adoptium API.

## Пути

- Проект: `D:\working WIN\LLM launcher` (bin: `madrid-launcher` + `installer` в `src/bin/`)
- Данные лаунчера: `%APPDATA%\LLM Launcher Madrid\` (config.json, game/, server/, runtime/{major}/, meta/forge installers cache, meta/icons, rustmc/repo, logs/launcher.log + server-*.log + game-*.log)
- Установка: `%LOCALAPPDATA%\Programs\LLM Launcher Madrid\madrid-launcher.exe`
- Ярлыки: рабочий стол (у пользователя Desktop в OneDrive — через `dirs::desktop_dir()`!) + `%ProgramData%`-Style Programs в Start Menu
- Тест-клон stevenarella: `D:\working WIN\rustmc-check\stevenarella` (патченный)
- Дистрибутив в корне проекта: `LLM Launcher Madrid.exe`, `Установить LLM Launcher Madrid.exe` (скрипт `scripts/make-dist.sh`: копирует launcher exe в assets/launcher-payload.exe → сборка installer c include_bytes → оба в корень)

## Архитектура (src/)

- `main.rs` — single-instance mutex `LLMLauncherMadrid-Instance` (2-й запуск фокусит окно), `server::bootstrap()`, при выходе `server::shutdown()` (stdin "stop" + kill)
- `config.rs` — Config с serde(default); `load()` стрипает BOM; `save()` атомарный (tmp+rename); auto_server=false по умолчанию; MADRID_MC="1.20.1"
- `app.rs` — Launcher state; задачи (InstallTask/ModTask/BuildTask/JavaTask/RustTask) с mpsc-каналами + AtomicBool cancel («Отменено»); poll() троттлит repaint (busy 120ms, сервер 400ms); кэши: mods_cache (2с), storage_size (4с), lan_ip, public_ip (OnceLock, api.ipify.org); detail_rx; build.json (своя сборка, плюсик в каталоге)
- `minecraft.rs` — Mojang manifest, install (ваниль), Forge через официальный installer headless `--installClient/--installServer` (build из promotions_slim.json); prepare_server_dir (eula=true, online-mode=false, autosave-interval=400, sync-chunk-writes), patch_server_props, sync_server_mods (клиент → сервер при каждом старте)
- `server.rs` — реальный хост MC-сервера: win_args.txt парсится ТОКЕНАМИ по пробелам (не целыми строками — иначе java падает на `-p`), CREATE_NO_WINDOW, stdin держится для "stop"; restart() для применения настроек; watcher-потоки
- `launch.rs` — build_args с inheritsFrom-merge, perf_gc_flags (Aikar G1GC по ядрам, ParallelGCThreads=cores≤12), HeapDump/ErrorFile; `launch(shared,cfg,id,connect: Option<&str>)` — quickPlayMultiplayer ТОЛЬКО при явном «На сервер»/«Зайти к другу»
- `mods.rs` — поиск Modrinth/CurseForge; KINDS: mod/shader/resourcepack/map/modpack; **facets: categories:{loader} ТОЛЬКО для mod** (для shader/resourcepack/modpack — только project_type+versions, иначе 0 результатов); пагинация offset (Modrinth limit=40, CF index=offset/20); модпаки: .mrpack → modrinth.index.json → докачка каждого файла по path + overrides; CF-паки → manifest.json + download-url API (ключ нужен); CF файлы качаются ПРЯМО с `www.curseforge.com/api/v1/mods/{id}/files/{fid}/download` (публичный CDN, без ключа); ProjectDetail (Modrinth /project/{slug}, clean_md рвёт markdown)
- `java.rs` — probe_all/pick_for (сканирует JAVA_HOME, PATH, vendor-папки, runtime/{major}); download_runtime (Adoptium zip → runtime/{major}); run_logged (скрытый вывод); JavaEvent::Done ОБЯЗАТЕЛЬНО эмитить
- `rustmc.rs` — stevenarella: git clone --depth 1 → patch_stevenarella (фикс vsync-close + GL Compatibility 2.1 вместо Core 3.2) → cargo build --release; launch с `--server 127.0.0.1:port --username nick` (server только если поднят); watcher: если exited <8s → подсказка про оверлей/драйвер
- `shared.rs` — Shared: cfg, versions, installed, java, game child+log, **mc-сервер** (running/busy/port/stage/error/log/started); журнал пишет в logs/launcher.log (файл, не очередь)
- `util.rs` — даты, autostart (reg run key), create_desktop_shortcut (PowerShell WScript.Shell; OneDrive-aware через dirs::desktop_dir), lock_single_instance, focus_window, dir_size, clear_cache (meta/forge + logs>7д + *.part), uninstall_everything (server shutdown → game kill → autostart → ярлык → data dir → bat-loop удаляет exe + rd Programs dir)
- `icons.rs` — свои stroke-иконки (Per-точки на 24×24 сетке), brand=Globe
- `theme.rs`/`widgets.rs` — Skin (alpha 0.15..0.99), свои слайдеры (**обязательно resp.mark_changed() при изменении!**), toggle, chip, card_frame, progress_bar, empty_state
- `views/` — play («Старт»: профиль+аватар-лицо скина, версии-чипы, Java-карточка с ин-апп загрузкой, правая колонка-карточка растянута на всю высоту), versions (Rust-издание НАВЕРХУ, loader-чипы сбрасывают каталог), mods (свап КАТАЛОГ/УСТАНОВЛЕННЫЕ, категории, сортировки ↓ скачивания/♥ популярность/↑ обновления/★ новые, сетка 158×214 / список (переключатель), «+ сборка», карточка проекта draw_detail, auto-load popular при пустых результатах, авто-обновление раз в 5 мин, отмена), server (ScrollArea, статус/аптайм/консоль сервера, код для друга ipify:port, «Зайти к другу», список модов сборки с копированием), settings (ник 3–16 валидация, Java-загрузка, хранилище+Очистить кэш, обои rfd, скин → game/skins + cfg.skin_file, порт сервера+память (мгновенный рестарт при отпускании слайдера), автозапуск с Windows, ярлык, полное удаление)
- `bin/installer.rs` — выбор папки (rfd pick_folder), чистка конфликтных exe (retry 15с с просьбой закрыть), payload из assets/launcher-payload.exe, ярлыки Desktop+StartMenu, self-delete через bat-loop в %TEMP%

## Важнейшие грабли (не наступи снова)

1. **windres ломается от пробелов в пути** — иконку exe встраивает build.rs: копирует assets/icon.ico в %TEMP%\madrid-launcher-icon (без пробелов) и запускает windres напрямую; winresource crate НЕ использовать
2. **BOM в config.json** — PowerShell Set-Content UTF8 пишет BOM → serde падает → лаунчер молча грузит дефолты; load() теперь стрипает, но редактируй конфиг ПИТОНОМ (utf-8 без BOM)
3. **Слайдер**: без `resp.mark_changed()` в widgets::slider `.changed()` всегда false → настройки откатывались
4. **stevenarella**: умирает через 2-3с с exit code **0xC0000005** внутри glutin `build_windowed` (без вывода в stderr) — краш драйвера/оверлеев при создании GL-контекста. Применены патчи: vsync-close → ignore, GL Compatibility (2,1) вместо Core (3,2). НА МОЕЙ машине всё ещё падает — вероятно оверлей ПО пользователя (Roblox/GeForce/RTSS). Следующие шаги: попробовать `with_multisampling(0)`, `with_srgb(false)`, обновить glutin/glium в форке, или wgpu-бэкенд
5. **win_args.txt** Forge-сервера — токенизировать по пробелам
6. **JavaEvent::Done** — если не эмитить, UI считает загрузку проваленной
7. **Кириллица в именах файлов** в bash —mv с русским именем отображается кракозябрами, но файл корректен (проверять через PowerShell)
8. `mv` в make-dist → `cp -f` (hardlink конфликт "same file")
9. Dir-сканы в UI только через кэш (dir_size каждый кадр = жуткие лаги настроек)
10. CF: поиск БЕЗ ключа невозможен (API требует ключ), скачивание — публичный CDN; карты — только CF; у Modrinth всё остальное без ключа

## Проверено E2E (реально работало)

- Сервер Forge 1.20.1-47.4.28: установка headless → boot «Done (9-18s)» → порт 25565 LISTENING
- Мод (Clumps) из game/mods → сервер mods/ → загружен Forge-ом (VersionCheck в логе)
- Java 17 JRE докачивается лаунчером сама (в системе только Java 8)
- Ярлык на OneDrive-рабочем столе с иконкой
- Обои/прозрачность/ресайз через DWM без бэкдропа
- stevenarella: clone+build ок (2.5 мин), но ЗАПУСК падает 0xC0000005 (см. грабли №4)

## Последние правки (20:48)

- Каталог: иконки webp включены (image crate +webp) — почти все аватарки теперь грузятся
- Сетка больше не уезжает за экран: строка поиска перенесена на две строки (чипы переносятся, поле отдельно), у grid жёсткий set_max_width(=ширина вьюпорта)
- Переключатель Сетка/Список удалён (владельцу не нужен) — остался только grid
- «Установленное» группируется по модпакам (meta/packs/installed.json, record_pack_files при установке пака): секции «МОДПАК · X» с кнопкой «Удалить пак», отдельно — «ОТДЕЛЬНЫЕ МОДЫ»; кнопка «Удалить все моды»
- Удаление/включение мода обновляет список мгновенно (сброс mods_at) + кэш тикает 1с
- Установщик: спам ошибок убран (NotFound игнор, сообщение о запущенном лаунчере — один раз, 15 попыток)
- Ник: 3–16 символов принудительно (save_cfg не сохраняет короткий)
- Модпаки: loaders-фильтр убран (паки не имеют loader=minecraft — иначе пусто), запись File() для живого списка

## Обновление 23:01 — stevenarella РАБОТАЕТ + магазин CF/Modrinth

- **stevenarella запуск исправлен**: причина краша 0xC0000005 — multisampling/sRGB в драйвере. Фикс в patch_stevenarella (rustmc.rs): `with_multisampling(0)` + `with_srgb(false)` + Compatibility 2.1 + vsync-ignore. Проверено: процесс жив, `Shader version: #version 150`
- **Логина в Rust-издании больше нет**: app.launch_rustmc поднимает локальный сервер (если выключен) и сразу коннектит (--server 127.0.0.1:port). Запускается игра напрямую
- CurseForge: описание проекта из /v1/mods/{id}/description (clean_html), баннер logo.url, modLoaderType=1 (Forge) и для модпаков, публичная ссылка CDN для скачивания без ключа. Ключ нужен только для поиска/карточек
- Modrinth: ProjectDetail.versions + compatible (есть ли 1.20.1/forge) — бейдж совместимости с Мадридом и список версий проекта в карточке
- Фильтры: chip_icon (Modrinth/CurseForge/Download/Heart/Refresh/Sparkle) — все чипы мгновенно сбрасывают results+offset и запускают поиск
- Профили: profiles.rs — изолированные mods-папки на профиль/сборку, переключатель на Старт, вкладка Сборка в рельсе

## Обновление 23:26 — CurseForge БЕЗ ключа + онлайн

- **CurseForge работает без API-ключа**: публичное зеркало api.curse.tools (v1/cf/*, та же схема v1). mods.rs cf_base(): с ключом — официальный api.curseforge.com, без — зеркало. Поиск модпаков/модов, файлы, download-url, описание — всё через него. serde aliases camelCase (downloadCount/latestFiles)
- **Онлайн как porthole**: вкладка Сервер → «Порт для друзей» (поле ввода) → «Готовый адрес сервера» (публичный IP + порт, копируется кликом). Друг вставляет адрес в «Код друга» → «Зайти к другу»
- **Rust-издание онлайн**: stevenarella знает протокол только до 1.18.2 (758), поэтому лаунчер поднимает отдельный VANILLA 1.18.2 сервер на порту server_port+1 и коннектит stevenarella к нему. НЕ путать с Forge-сервером Мадрида (1.20.1)
- Логи: game_log_tail/mc_log_tail(240) — клонирование только последних строк (оптимизация)

## Обновление 23:45

- CurseForge: убрана оставшаяся проверка «нужен ключ» в search() — каталог снова полный без ключа (зеркало api.curse.tools)
- Текст перестал вылезать за рамки: hit_card клипится по ячейке (with_clip_rect), заголовки ≤18 символов; длинные подписи/ошибки — egui::Label::wrap(); строки установленных модов клипятся тоже
- Подсказка для карт: «карты на CurseForge — зеркало без ключа»

## Обновление 00:02

- «Играть» теперь САМ ставит клиент Мадрида (Forge), если его нет, и открывает игру после установки (InstallTask.auto_launch). Ошибка «Нет файла версии 1.20.1.json» ушла — она возникала, когда стоял только сервер
- CurseForge-паки ставятся БЕЗ ключа: файлы резолвятся через /files/{fid} (mirror) → downloadUrl (edge.forgecdn.net), fallback — публичный www.curseforge.com download
- Rust-издание: скачивание исходников — ZIP с GitHub (codeload), git больше не нужен; cargo ищется в PATH и %USERPROFILE%\.cargoin; понятная ошибка, если Rust не установлен
- Текст: клиппинг карточек, wrap длинных подписей

## Обновление 00:30 — самолечение сборки

- Краш «Provider toni.missingmodschecker.NeoBootstrap not found» найден в логах: битый/чужой jar в сборке. Теперь launch.rs heal_broken_mod: при exit code 1 парсит лог, находит jar с этим provider-путём и переименовывает в .disabled + сообщение «нажми Играть снова»
- Fabric-паки больше не встают в Forge-профиль: mrpack с dependencies.fabric* и CF manifest modLoader==4 отклоняются с понятным сообщением
- CF-паки: имена файлов берутся из API fileName (группировка по пакам теперь точная)
- Старт: ScrollArea + карточка «Установка клиента» со стадией/процентом/Отменить
- Сервер: «Готовый адрес» — явная кнопка «Скопировать адрес» + LAN-fallback, если ipify недоступен

## Обновление 00:35 — модпак = профиль

- Установка модпака создаёт ПРОФИЛЬ (profiles::create_modpack_profile, kind Modpack) и переключается на него: моды пака изолированы в своей папке, живут в meta/profiles/{id}/mods, в game/mods попадают только при активации. Никаких «3 отдельных модов»
- Пак диктует версию: mrpack dependencies.minecraft + forge → ModEvent::PackMeta → профиль обновляет mc_version/loader → «Играть» ставит нужный Forge сам (target_version теперь читает АКТИВНЫЙ профиль)
- CF-модпаки: показываются ВСЕ загрузчики (фильтр modLoaderType убран) — несовместимость проверяется при установке с понятным отказом (Fabric-паки на Forge-профиль не встанут)
- Пагинация каталога: кнопка «Показать ещё» внизу (Modrinth limit=40, CF pageSize=20, exhausted когда страница < лимита)
- CF-детали: description через зеркало /mods/{id}/description (clean_html), лого-баннер, счётчик загрузок (serde alias downloadCount)

## Обновление 01:20 — Pomme (нативный Rust-клиент) + авто-моды

- **Pomme**: нативный Rust+Vulkan клиент с ВСТРОЕННЫМ однопользовательским сервером (SteelMC) — без Java, без логина, без сервера вообще. Скачивается портативным ZIP (32 МБ, один pomme-client.exe) из GitHub releases в rustmc/pomme. Кнопки «Скачать Pomme» / «Играть» на вкладке Версии. Проверен запуск: живёт
- **Авто-моды при Играть**: launch.rs перед запуском копирует из server/mods все jar-ы, которых нет у клиента — «missing mods» на своём сервере больше не появляются
- CF-карточки: описание пака видно прямо в сетке (summary 44 символа)
- stevenarella остаётся опцией (протокол до 1.18.2, нужен vanilla-сервер 25566)

## Обновление 01:40 — финал UX

- Pomme запускается БЕЗ логина: offline-флаги --username/--uuid/--access-token 0 + отдельный --game-dir
- «Удалить модпаки» (Установленные): сносит все Modpack-профили с их хранилищами, активный откатывается на Основную сборку
- При установке модпака клиент нужной версии качается СРАЗУ (не на «Играть»); install() пропускает уже скачанное — дубликатов нет
- «Играть» на профиле пака: клиент уже стоит → запуск напрямую

## Обновление 2026-11-16 — v0.5.0: модпаки в одно нажатие, комнаты, анимации

**Архитектура (новое):**
- `loader.rs` — чистая логика загрузчиков: `Loader {Vanilla, Forge, NeoForge, Fabric, Quilt}`, `Spec {mc, loader, version}`, разбор mrpack/CF-манифестов, `pick_installed`, `java_fits` (Forge и MC ≤1.16 — точная Java, остальным можно новее). Тесты внутри.
- `net.rs` — ОДИН multi-thread tokio-рантайм + ОДИН reqwest-клиент (gzip/br/zstd, keep-alive). Все фоновые задачи через `net::spawn`. `minecraft::http_client()` остался для изолированных runtime (server.rs).
- `pomme.rs` вместо `rustmc.rs`. stevenarella УДАЛЁН целиком (и vanilla-1.18-сервер, и vertdll.dll в установщике). Pomme запускается с `--quick-access-singleplayer <последний мир>`; официальный релиз Pomme сам требует вход Microsoft один раз (`cfg!(not(debug_assertions))` в их коде) — НЕ обходим.
- `room.rs` — комнаты: код из 5 цифр → topic `llmmadrid-<md5[..20]>` на публичном ntfy.sh (лимит ~250 постов/день/IP → пост при входе/изменении/выходе + пульс раз в 4 мин; приём одним стрим-соединением `/json?since=`). Участник «пропал» через 10 мин тишины. Хост вписывает порт → друзья видят IP:порт (LAN, если внешний IP общий), «Скопировать», «Зайти» (`play(Some(addr))`), фоновая TCP-проверка доступности. Живой тест: `cargo test room:: -- --include-ignored`.
- `upnp.rs` — SSDP+SOAP проброс TCP-порта при хостинге, закрытие при выходе/закрытии окна (`on_exit`). `home_ip()` из `route print -4` (интерфейс с реальным частным шлюзом) — VPN-адаптер не выигрывает. Внешний IP: ipify через домашний интерфейс (`local_address`) или WAN IP роутера. CGNAT/двойной NAT — честное предупреждение в UI.
- Сборки = отдельные папки: default → `game/`, остальные → `meta/profiles/{id}/` (mods, config, saves, options). `Config::instance_dir()`; версии/библиотеки/ассеты общие в `game/`. `--gameDir` = папка сборки. Переключение ничего не копирует. Одноразовая миграция `profiles::migrate_to_instances` (флаг `instances_v2`, field-level serde default!).
- `profiles.rs`: кэш profiles.json по mtime, кэш списков модов по mtime папки, `upsert_modpack_profile` (тот же пак → та же сборка, миры/скриншоты/options сохраняются), `delete_all_builds` (все сборки + папки + неиспользуемые версии загрузчиков + meta/packs).

**Исправленные корни багов:**
1. Модпаки: Fabric/Quilt/NeoForge отклонялись; CF-манифест читался по несуществующим полям (каждый CF-пак = 1.20.1 Forge); пак качался в общую game/mods и плодил по 2 профиля. Теперь любой загрузчик и любая версия MC, точная версия загрузчика из пака, файлы параллельно (12), CF-файлы через публичный редирект (имя из конечного URL, без API-запроса на каждый файл).
2. «Играть» всегда ставил Forge и мог запустить ваниль. Теперь `launch_game/play` берёт версию под активную сборку; нет версии/загрузчика/Java → ставит сам и запускает.
3. Java: `required_for("1.21")` давал 8. Java докачивается автоматически внутри установки.
4. CF-каталог: `index` = смещение элементов (было «номер страницы» → дубли); sortField: 2 популярность, 3 обновления, 6 скачивания, 11 новые (было 5 = автор); гонка запросов (новый фильтр игнорировался, пока шёл старый) — теперь номер запроса `gen`, устаревшие ответы идут только в кэш; кэш страниц 10 мин + предзагрузка следующей; поиск при наборе (420 мс); бесконечная прокрутка; зеркало api.curse.tools требует User-Agent (без него Cloudflare 302).
5. Описание проекта рисовалось в родительском ui ниже экрана — его не было видно. Теперь карточка со слайдом, галерея, версии, совместимость с активной сборкой; устаревший ответ другой карточки не подменяет текущую.
6. Скорость: чтения profiles.json и папки модов на каждом кадре убраны; иконки — общий рантайм, диск-кэш, декод вне UI-потока, уменьшение до 256 px, лимит текстур; Java-пробинг кэшируется на 60 с; библиотеки качаются параллельно (16); `download_file` с повторами и уникальным `.part`; release `opt-level = 3`.
7. `heal_broken_mod`: `.lines().flatten()` мог зависнуть навсегда + `unwrap` (abort в release).
8. Pomme запускался дважды (дублированный блок).

**Дизайн/анимации (impeccable animate):** `theme::{ease_out, arrive, tween, travel, reduced_motion}` с учётом «Показывать анимацию в Windows». Переход вкладок 240 мс, скользящая подсветка меню, каскад карточек каталога (шаг 18 мс, ≤0,3 с), подъём карточки с тенью при наведении, скелетоны, плавный прогресс + бегущий сегмент при неизвестном размере, чипы заливаются от центра, кнопки «проседают» при нажатии, тосты всплывают и гаснут.

**Грабли:** heredoc с тройными кавычками в Bash-инструменте ломается — писать скрипты файлами; Git Bash превращает аргумент `//...` в `/...` (MSYS) — ставить `MSYS_NO_PATHCONV=1`. У владельца VPN `happ-xray` (172.19.0.1) перехватывает маршрут; роутер 192.168.1.254 UPnP не отдаёт → для игры через интернет порт открывать вручную, в одной сети работает сразу.

## Обновление 2026-11-17 — v0.6.0: запуск без крашей, скорость, умный поиск

**Краши со скриншотов (корень найден, проверено живым запуском Forge до главного меню):**
1. «Modules minecraft and _1._20._1 export package …» (Forge 1.17+/NeoForge, Crash Assistant) — в `-DignoreList` подставлялось `1.20.1-forge-….jar`, а в classpath лежит общий `versions/1.20.1/1.20.1.jar` → он грузился вторым модулем. `launch::fix_ignore_list` дописывает реальное имя jar. Ломало ВСЕ Forge/NeoForge-сборки (Fabric работал).
2. Проверка модов ДО запуска (`launch::preflight_mods` / `incompatible`): Fabric/Quilt-моды в Forge/NeoForge-сборке, битые zip, jar с объявленным, но отсутствующим ModLauncher-сервисом (`missingmodschecker` → «Provider … NeoBootstrap not found») → `.disabled` (обратимо). В основной сборке «Мадрид» лежало ~300 Fabric-модов — они отключены.
3. Самолечение после краша понимает «Mod candidate X contains a corrupt or misconfigured toml» и сервис-файлы; авто-копирование server/mods → клиент не возвращает отключённые моды; sync клиент → сервер удаляет с сервера отключённое в клиенте.
4. Недостающие зависимости (`deps.rs`): лог игры парсится на лету (`MissingParser`: Forge «Mod ID: 'x', Requested by: 'y'…», Fabric «requires … (id), which is missing!»). Лаунчер закрывает экран ошибки и ищет мод: обязательные зависимости мода-заказчика по SHA-1 (Modrinth `/version_files`) и по отпечатку CurseForge (MurmurHash2 без пробельных байт, `/fingerprints/432`), отключённый «двойник» под другой загрузчик (тот же проект), slug, поиск. Кандидат оставляется ТОЛЬКО если в его `[[mods]]`/fabric.mod.json есть нужный id (учтены секции `[[dependencies]]` и `#комментарии` в toml). Затем сам перезапускает игру; один набор id — одна попытка за сессию. Проверено: «Мадрид» докачал 9 модов цепочкой и дошёл до меню.
5. FTB-моды для Modrinth-паков (`ftb.rs`, экран «Modrinth does not have the FTB mods!»): по `config/ftbquests` и ссылкам `mc-mods/ftb-…` в конфигах (FancyMenu) докачиваются FTB Quests/Teams/Library/XMod/Chunks/… с CurseForge под версию/загрузчик пака + их обязательные FTB-зависимости. При установке пака и при «Играть» на уже стоящем паке.

**Загрузка (`net.rs`):** `fetch_to` — первый запрос Range 0..2 МБ; если 206 и файл больше — остальное до 8 параллельными диапазонами по смещениям файла, докачка оборванного сегмента. Ассеты 64 в полёте, библиотеки 32, файлы паков 24. Глобальный счётчик байт → `SpeedMeter` → «12,4 МБ/с» слева от «5/120», в установке клиента и Java.
- ГРАБЛИ: CDN CurseForge (forgecdn) отвечает 404 на ЛЮБОЙ Range-запрос → для forgecdn/curseforge.com диапазоны выключены (`net::ranges_supported`) + откат на обычный GET при не-2xx. Modrinth CDN не понимает suffix-range `bytes=-N` (501).
- Прямые ссылки `edge.forgecdn.net/…%20…` (пробел в имени) = 404 → `cf_file_url` берёт публичный редирект www.curseforge.com.
- Зеркало CF переехало: `https://api.curse.tools/v1` (старый `/v1/cf/…` редиректит на http).

**Java (`java.rs`):** распаковка в `runtime/{N}.unpacking` → проверка `java -version` → атомарный rename + маркер `.madrid-ok`; полу-распакованные рантаймы удаляются; одна загрузка на мажор; источники Temurin JRE → Temurin JDK → Microsoft OpenJDK (11/17/21/25); нет мажора (16) — ближайший LTS, который принимает загрузчик. UI показывает Java, которая реально запустит сборку (`java::fitting_cached`), а не самую новую.

**Каталог:** категории (`categories.rs`, статические таблицы Modrinth slug / CF id с русскими названиями) — «Категории: все» справа от «Новые», поиск внутри списка, поиск каталога идёт внутри выбранной категории. На карточке — категории и у модпаков «405 модов · 1,6 ГБ» (`packinfo.rs`: оглавление zip по Range; CF без Range — архив ≤24 МБ целиком, в открытой карточке ≤100 МБ, иначе «архив N МБ»; кэш meta/packinfo.json). Иконки/инфо грузятся только для видимых карточек.
- Умный поиск (`smart.rs`): словарь RU→EN («мини карта»→minimap), транслит («проминсе»→prominse), неверная раскладка («зкщьштутсу»→prominence), префикс против опечаток («promin»), параллельные запросы + переранжирование Jaro–Winkler. Modrinth при введённом тексте сортирует по relevance. CJK вырезается (`mods::displayable`) — в шрифтах его нет, CJK-шрифт = +20 МБ RAM.

**FPS:** игра стартует с ABOVE_NORMAL приоритетом; java.exe прописывается на дискретную видеокарту (HKCU UserGpuPreferences, GpuPreference=2); клиентские JVM-флаги без AlwaysPreTouch (+StringDedup, CodeCache 400M, без PerfData); новая сборка получает options.txt с vsync off / maxFps 260 (только если файла нет). Лаунчер сворачивается при старте игры, не рисует кадры в фоне, разворачивается после выхода.

**Память/диск:** tokio 2–4 потока со стеком 1 МБ; иконки 192 px, ≤240 текстур; обои ≤1920; `util::trim_memory` (SetProcessWorkingSetSize) через 5 с после старта, при сворачивании и потере фокуса → в простое ~5 МБ «Память» в диспетчере (пик при старте ~105 МБ). Прогресс-события не дёргают перерисовку на каждый сетевой пакет. `util::prune_data` при старте: кэш картинок ≤40 МБ (хранятся уменьшенные PNG, было 105 МБ), последние 12 логов каждого вида.

**Текст:** хинтинг без субпиксельного сдвига (`FontTweak`, `subpixel_binning=false`), мелкие кегли подняты (`theme::readable`), `widgets::text_fit` (обрезка по реальной ширине с «…») и `widgets::lead_text` (подпись ряда не залезает под кнопки) — убраны наложения в карточках, строках модов/версий/сборки, прогресс-карточках; колонки «Сервер» и «Старт» не вылезают за поле; «▾»/«✕» заменены (нет глифов).

**Отладка:** `MADRID_SECOND_INSTANCE=1` — вторая копия рядом с запущенной; `MADRID_DEV_TAB=play|versions|build|mods|modpacks|server|settings` (+ `MADRID_DEV_QUERY`) — сразу открыть вкладку (скриншот окна через PrintWindow). Живые тесты: `cargo test live_ -- --ignored --nocapture` (поиск/packinfo/загрузка/FTB/Java/deps/`live_forge`/`live_e2e`; `MADRID_FULL_BOOT=1` — `live_forge` ждёт главное меню). В Bash-инструменте heredoc с python ломается на кавычках/слешах — патч-скрипты писать файлами в scratchpad.

## Обновление 2026-11-17 (день) — v0.7.0: Sad Launcher, скорость, удаление, сайт

**Переименование:** «LLM Launcher Madrid» → «Sad Launcher». Данные: %APPDATA%\Sad Launcher. config::migrate_legacy() один раз переименовывает старую папку и правит абсолютные пути в json (config, meta/*.json); пока rename не удался (старая копия держит файл) — data_dir() продолжает работать со старой папкой. util::migrate_legacy_names(): старый ярлык и Start-меню удаляются, автозапуск переносится на новое имя. Бинарь sad-launcher.exe (package sad-launcher), установщик ставит в Programs\Sad Launcher и чистит старую установку. Мьютекс LLMLauncherMadrid-Instance, topic комнат llmmadrid-*, переменные MADRID_* — НЕ переименованы (совместимость). Шапка окна — только надпись SAD LAUNCHER, без логотипа (желание владельца). Иконка exe/окна — белая S на чёрном (assets/icon.png/.ico, сгенерирована PIL; окно получает её через with_icon).

**Скорость паков (unpack.rs):** архив пака больше не распаковывается целиком во временную папку и не копируется второй раз. Индекс читается прямо из zip, overrides/client-overrides пишутся сразу в сборку в 2–8 потоков (у каждого свой ZipArchive, крупные файлы первыми) ПАРАЛЛЕЛЬНО со скачиванием модов (spawn_blocking). Файлы, которые перекрыты overrides, не качаются. LocalFiles: моды/ресурспаки из других сборок с тем же именем и размером (+SHA-1 из mrpack) хардлинкаются вместо скачивания. CF: одним POST /mods/files узнаются имена/размеры — уже лежащие файлы пропускаются без запроса к CDN. Пакет файлов 32 параллельно. Java JRE распаковывается тем же параллельным unzip. Замер (cargo test live_pack_speed -- --ignored --nocapture, MADRID_PACK=slug): Additive, 54 файла, 51 МБ за 4,4 с — упор в канал (~12 МБ/с), overrides пишутся за миллисекунды.

**Удаление (remove.rs):** всё удаление — в фоне. queue(title, path) мгновенно переименовывает папку в data/.trash (на том же диске) → сборка пропадает из списков сразу, затем поток sad-remove удаляет файлы в 2–8 потоков (снимая read-only). queue_files — пачка файлов без переименования (удалить все моды). Карточка прогресса справа снизу (draw_remove_card): «Удаление · ещё N», название, бар, «N / M файлов · освобождено X», появляется со scale 0.92→1, после «Удалено» висит ~1,8 с и сжимается. После каждого элемента списки/хранилище обновляются (finished_count). Остатки .trash после прерванного удаления дочищаются при старте (sweep_leftovers). delete_all_builds больше не считает dir_size в UI-потоке.

**UI:** на карточках каталога «+» → «Добавить» (подсказка), у паков «Установить»; в строках модов сборки «×» → «Удалить»/«Точно?», точка-переключатель с подсказкой и подсветкой. Баг: status_dot аллоцировал место и уезжал на строку ниже — теперь paint_status_dot рисует в своём rect. Карточки каталога: scale (появление с 96 %, +1,5 % при наведении, −1 % при нажатии).

**Сайт (site/):** index.html + styles.css + app.js, без библиотек. Hero — процедурный воксельный рельеф на canvas (низкое разрешение + pixelated, ~20 fps, пауза вне экрана/во вкладке в фоне), Playfair Display (заголовки) + Onest, стекло через backdrop-filter. Container scroll (21st.dev) — CSS animation-timeline: view(); spotlight-карточки, магнитная кнопка, CountUp, одна бегущая строка (reactbits). Скачивание: Windows → download/SadLauncherSetup.exe (make-dist.sh копирует), Linux/macOS — «в разработке» (код лаунчера пока Windows-only: реестр, DWM, FileExt::seek_write). Слабые устройства: класс .lite (≤4 ядра / ≤4 ГБ / save-data) убирает blur и анимацию рельефа; content-visibility для секций; prefers-reduced-motion/transparency. Запуск: «Запустить сайт.bat» в корне (python http.server 8080 → py → npx serve). Скриншоты лаунчера: MADRID_SECOND_INSTANCE=1 MADRID_DEV_TAB=… + PrintWindow (dev-копия НЕ мигрирует папку и НЕ трогает ярлыки).

## Открытые задачи

- Хостинг сервера для паков не-Мадрид (сервер лаунчера — Forge 1.20.1 Мадрид)
- Друзья могли бы сообщать хосту результат проверки доступности (сейчас её видит только тот, кто заходит)
- CF-поиск без ключа — если понадобится, искать публичный прокси (api.curse.tools и т.п., ненадёжно)
- Full E2E: клиент Forge + вход на сервер (нужна реальная установка клиента 1.20.1, ~500MB)
- Возможно: экспорт/импорт сборки файлом для друзей (сейчас — копирование списка текстом)
- Порт лаунчера на Linux/macOS (сайт уже показывает кнопки «Скоро»): убрать WinAPI за cfg(windows)
- В корне лежат старые LLM Launcher Madrid.exe / Установить LLM Launcher Madrid.exe — можно удалить

## Команды

```bash
cargo check / cargo build --release        # сборка
bash scripts/make-dist.sh                  # launcher + installer в корень проекта
# запуск для теста:
./"Sad Launcher.exe"
# конфиг пользователя:
%APPDATA%\Sad Launcher\config.json  # править ТОЛЬКО питоном (utf-8, no BOM)
# сервер-логи:
%APPDATA%\Sad Launcher\logs\server-*.log
```

## Вкусы владельца

Всё на русском, чёрно-белый минимализм, без консольных окон (CREATE_NO_WINDOW 0x08000000), минимум файлов на диске (кэш чистится, «облако» = Mojang/Forge CDN), много кликабельности, живой прогресс (моды «пролетают» списком), ничего не должно запускаться/подключаться без явного действия пользователя. Скриншоты с взаимодействием НЕ делать, если владелец играет в fullscreen Roblox (проверять через файлы/логи).

## Обновление 2026-11-17 (вечер) — восстановление и Antigravity

- Папка проекта была стёрта целиком (`src/` ни разу не коммитился). `src/` восстановлен из логов сессий поверх резервной копии от 16.11; сборка чистая, 57 тестов проходят, `make-dist.sh` пересобрал exe и установщик. Сверить с оригиналом нельзя. Правило: после значимой работы предлагать коммит и пуш `src/`.
- Для Antigravity: `ANTIGRAVITY.md` (полная инструкция), `AGENTS.md`, `GEMINI.md`, `antigravity/` (31 скилл + allskill, 9 workflows, правило always-on, `setup-antigravity.ps1`). Скрипт раскладывает в `.agent/` и `.agents/` (в `.gitignore`).
- `rustmc.rs` удалён ещё раньше (Pomme вместо stevenarella); `assets/vertdll.dll` больше не нужен.

## Обновление 2026-11-17 (поздний вечер) — выравнивание UI, «Игра с другом», прозрачные кнопки, автообновление

- **Прозрачные кнопки и семантические цвета (`theme.rs`, `widgets.rs`, `views/mod.rs`, `app.rs`)**: убрана сплошная белая заливка с `Btn::Primary`, `play_button`, выбранных `chip`, `badge(..., true)` и экрана создания ника — теперь кнопки прозрачные стеклянные с чёткой рамкой и полностью читаемым текстом (в стиле inline-кнопок Telegram). Добавлены только два акцента: `Skin::green()` (в одной сети / сервер доступен / готово) и `Skin::red()` (ошибки / не отвечает).
- **Ползунки и скроллбары (`theme.rs`, `widgets.rs`)**: скроллбары больше не перекрывают текст (`floating_allocated_width = 9.0`, видимые прозрачные ползунки с `dormant_handle_opacity = 0.36`, `active_handle_opacity = 0.72`), усилена дорожка и ползунок `widgets::slider`.
- **Выравнивание строк и наложения (`widgets.rs`, `app.rs`, `views/*.rs`)**:
  - Исправлен `widgets::lead_text` (высота `0.0` давала сдвиг текста вниз внутри `right_to_left(Align::Center)` — теперь минимум `24.0` px для однострочных рядов).
  - Выровнены: строки путей в `draw_about` (`Данные`/`Игра`/`Журналы`), нижняя строка `v0.7.0` + `О лаунчере` и профиль в боковой панели (`app.rs`), фильтр категорий в `mods.rs` (`compact_input` + `Truncate`), поиск версий в `versions.rs` (выделенная строка `32px`), формы/кнопки в `build.rs` (перенос на `960px` без переполнения), профиль и карточка состояния в `play.rs`, `field_row` и порт в `settings.rs`.
- **Вкладка «Игра с другом» (`views/server.rs`, `room.rs`, `launch.rs`, `util.rs`, `app.rs`)**:
  - Секция «Игра с другом» поднята наверх, «Локальный сервер Мадрид» убран ниже; строки действий («Создать/Войти», «Раздаю игру · порт», два адреса, «Адрес друга · Зайти») идут строго в одну полосу с расчётом ширины под минимум `960px`.
  - При вводе порта сразу показываются **два адреса**: локальный (`Сеть`) и внешний (`Интернет`). Все коды и адреса отрисованы через `widgets::copyable_pill` (можно выделить мышкой и скопировать `Ctrl+C`), а кнопки используют `util::copy_text` с надёжной записью в Win32-буфер (`OpenClipboard` с повторами + UTF-16).
  - Подсветка **зелёным** при нахождении в одной сети (`room::same_network`) и доступности сервера; живая строка процессов и **красные** ошибки (`FriendState` в `app.rs`).
  - Исправлен `launch::build_args`: для Minecraft `< 1.20` передаются `--server` и `--port` (раньше подключение друга для версий ниже 1.20 игнорировалось); убрано затирание пустого `host_port_buf` при редактировании порта; исправлен вложенный `kv_row` у списка модов.
- **Автообновление с GitHub (`updater.rs`, `bin/installer.rs`, `config.rs`, `scripts/make-dist.sh`, `app.rs`)**:
  - При старте в фоне проверяется GitHub Releases (`/releases/latest`) и fallback `site/version.json` (`raw.githubusercontent.com`). Если вышла более новая версия, снизу справа появляется компактная карточка с размером обновления и тремя кнопками: «Обновиться», «Напомнить позже», «Не показывать» (`Config::skip_update_version` с `serde(default)`).
  - «Обновиться» скачивает установщик в `%TEMP%\sad-launcher-update.exe` (проверка домена GitHub + HTTPS + заголовок `MZ` + размер) и запускает с `--update <current_exe>`: установщик ждёт закрытия лаунчера, заменяет только exe-файл (папка `%APPDATA%\Sad Launcher` с ником, сборками и мирами не трогается) и запускает обновлённый лаунчер. `make-dist.sh` автоматически пишет `site/version.json`.
- **Навигация мышью, быстрая промотка и бинды (`app.rs`, `views/mods.rs`, `views/versions.rs`, `views/build.rs`)**:
  - Боковые кнопки мыши `Extra1` / `Extra2` (и `Alt+Left` / `Alt+Right`) листают историю переходов (`nav_history`: вкладки, Каталог/Установленные, карточка мода) на шаг назад/вперёд.
  - Зажатие или клик колёсика мыши (`PointerButton::Middle`) включает быструю плавную автопромотку с круглым индикатором-якорем (`handle_middle_scroll`).
  - Дефолтные бинды: `Ctrl+1..7` (вкладки), `Ctrl+Tab`/`Ctrl+Shift+Tab`, `Ctrl+F`/`Ctrl+K` (фокус в поиск), `F5`/`Ctrl+R` (обновить), `PageUp`/`PageDown`/`Home`/`End` (промотка), `Esc` (закрыть оверлей/сбросить подтверждение).
- **Вкладка «Эффекты» (`effects.rs`, `views/effects.rs`, `launch.rs`, `app.rs`)**:
  - Новая вкладка слева (`Tab::Effects`, `Icon::Sparkle`) с тремя секциями: Освещение и видимость в жидкостях (`Fullbright`, `No Fluid`, `Custom Fog`), Очистка экрана (`No Fire`, `No Bad Effects`, `No Hurt Cam`, `No Weather`, `No Particles`, `No Overlay`), Освещение мира (`World Time`, `Ambience`).
  - В чистом Rust сохраняется в `%APPDATA%\Sad Launcher\effects.json`, при нажатии «Применить сейчас» и каждом запуске (`launch::launch`) патчит `<instance>/options.txt` (`gamma`, `damageTiltStrength`, `darknessEffectScale`, `hideLightningFlashes`, `particles`, `resourcePacks`) и генерирует `<instance>/resourcepacks/SadEffects.zip` (низкий/скрытый огонь, прозрачная тыква/рыхлый снег/тотем/дождь/снег/вода/частицы). Для эффектов, требующих хука рендера (1.19+ гамма, туман, Blindness/Nausea, время суток, цвет неба), честно отмечено, что нужен мод, и есть 1-клик поиск на Modrinth.
- **Правило протокола**: во всех файлах правил (`AGENTS.md`, `ANTIGRAVITY.md`, `allskill`) закреплено обязательное обновление `RRR.md` и автоматический `git commit` + `git push` в `main` после проверок.
- **Грабли**: в изолированной Win32-сессии агента `CopyFromScreen` возвращает `"The handle is invalid"`, а `PrintWindow` на прозрачном OpenGL (`glow`) окне даёт белый буфер — сборка проверена живым запуском (`Sad Launcher.exe` с `MADRID_SECOND_INSTANCE=1`) без падений.

## Обновление 2026-11-17 (ночь) — моды эффектов в 1 клик, автоматизация, SVG-иконки, автопромотка, сайт и Botkeep

- **Эффекты в реальном времени и 1-клик загрузка модов (`effects.rs`, `views/effects.rs`, `app.rs`)**:
  - Наверху вкладки «Эффекты» показывается живой статус Minecraft (запущен / не запущен, активен / свёрнут) и пояснение для онлайн-игры (`options.txt` + `SadEffects.zip` применяются сразу, ресурспак обновляется по `F3+T`, новые `.jar` моды — при перезапуске).
  - У каждого эффекта бейдж реальной готовности (`Готово` в зелёном цвете, `Нажми Применить` или `Нужен мод` / `Нужен Fabric/Forge`), а для эффектов с клиентскими модами (`Gamma Utils`, `Clear Water/Lava`, `No Fog`, `CleanView/NoBlindness`, `Time Changer`, `Sky Aesthetics/Ambient`) есть кнопка **«Скачать мод»** прямо в строке и общая кнопка **«Скачать моды (N)»** без перехода в каталог.
- **Anti-AFK и автокликер только внутри Minecraft (`automation.rs`, `shared.rs`, `launch.rs`)**:
  - `Shared::game_pid` хранит PID запущенного процесса игры; фоновый поток проверяет активное окно через `GetForegroundWindow` + `GetWindowThreadProcessId` + `GLFW30/LWJGL`.
  - **Защита от АФК**: настраиваемый интервал `5–300` с (по умолчанию 60 с), микродвижение вперёд (`W`) скан-кодом в фокусе или `PostMessageW` в HWND игры (никогда не печатает в сторонние окна), живой обратный отсчёт.
  - **Автокликер**: ЛКМ / ПКМ, `1–25` кл/с, интерактивный захват бинда («Задать бинд» — клавиатура `F1..F12`, `A..Z`, `0..9`, боковые кнопки мыши `XButton1/2`, `MButton`). Кликает строго пока окно Minecraft активно и автоматически ставится на паузу при сворачивании/переключении.
- **Исправление автопромотки колёсиком (`app.rs`)**:
  - Найдена причина зависания круга автопромотки (`^ • v`) без скролла: `handle_middle_scroll` вызывался в конце `update()` **после** `view_*`, поэтому `smooth_scroll_delta.y` сбрасывался до чтения виджетами `ScrollArea`. Разделено на `pump_middle_scroll` (до рендера вкладок) и `draw_middle_scroll_anchor` (оверлей поверх).
- **Векторные SVG-иконки (`assets/icons/*.svg`, `icons.rs`, `views/mods.rs`, `views/server.rs`, `views/effects.rs`, `widgets.rs`)**:
  - Все 31 мелкие и системные иконки перенесены в настоящие `.svg` в `assets/icons/` (включая волшебную палочку с лучами и искрами `effects.svg`) и рендерятся встроенным zero-dep SVG-парсером в `icons.rs` со скруглёнными концами штриха и кривыми Безье. Заменены текстовые стрелки в карточках модов (`Icon::ArrowDown`) и кнопках (`Icon::ArrowLeft`, `Icon::Copy`, `Icon::Search`, `Icon::Keyboard`).
- **Выравнивание полей и капсул (`widgets.rs`, `views/server.rs`)**:
  - Исправлен баг `compact_input` / `copyable_pill`, где текст смещался в левый верхний угол прямоугольника: добавлено `TextEdit::align(Align2::LEFT_CENTER)` с `min_size` на всю высоту, уменьшена высота до `26–28` px, а `copyable_pill` подгоняет ширину плотно по ширине текста.
- **Сайт, домен, автоустановка и хостинг (`site/`, `.github/workflows/release.yml`, `config.rs`, `updater.rs`, `erootstars`)**:
  - Задан `SITE_URL = "https://rkt5jy.bot-keep.xyz"` (и разрешён в `updater.rs` + `make-dist.sh` для автообновлений даже при приватном репозитории).
  - Убраны ручные инструкции сборки на Linux/macOS: добавлены скрипты `site/download/install-macos.command` (установка `.app` в Программы) и `site/download/install-linux.sh` (бинарник + `.desktop`), а также `.github/workflows/release.yml` для кросс-сборки Windows/macOS/Linux.
  - На сайте (`site/index.html`, `styles.css`, `app.js`) подключены GSAP + ScrollTrigger (с безопасным fallback без CDN), добавлен поиск и раскрытие **FAQ** в стиле `eroot.shop` (инверсный sweep-фон, сниппеты поиска, ссылки на ответы `#faq-*`, клавиатурная навигация). Проверено в браузере на 1440 px и 375 px с 0 ошибками консоли.
  - В репозитории `simsislam89-crypto/erootstars` (`D:\working WIN\eroot-store`) добавлена маршрутизация по `Host`: `rkt5jy.bot-keep.xyz` / `*.bot-keep.xyz` отдаёт `launcher_site/` со своим CSP, `eroot.shop` остаётся магазином, `config.py` также читает `SERVER_PORT` панели Botkeep.
- **Грабли**: в Windows `TextEdit::singleline` без явного `min_size` + `align(Align2::LEFT_CENTER)` игнорирует вертикальное выравнивание родительского layout; `cp` в `Sad Launcher.exe` падает с `Device or resource busy`, если лаунчер открыт во время `make-dist.sh` — добавлен мягкий fallback в `Sad Launcher.new.exe`.

## Обновление 2026-11-17 (поздняя ночь) — лимит Botkeep, 7 интерактивных компонентов сайта, автокликер в мс, Анти-АФК до 60 мин и адаптивная сетка каталога

- **Исправление ошибки импорта GitHub на хостинге Botkeep (`erootstars`, `webhook_server.py`, `.gitignore`, `tests/test_v139.py`)**:
  - На скриншоте панели Botkeep выяснилось жёсткое ограничение GitHub-импорта: **не более 1000 файлов, 4 МБ на один файл и 20 МБ суммарно на репозиторий**. Файл `launcher_site/download/SadLauncherSetup.exe` (`17.66 МБ`) превышал лимит в 4 МБ и раздувал `erootstars` до `33.91 МБ`.
  - Решение: `launcher_site/download/SadLauncherSetup.exe` удалён из `erootstars` и добавлен в `.gitignore` (`launcher_site/download/*.exe`), репозиторий `simsislam89-crypto/sad-launcher` переведён в `public` с домашней ссылкой `https://rkt5jy.bot-keep.xyz`, а `launcher_host_router` в `services/webhook_server.py` отдаёт `302 Found` на `https://github.com/simsislam89-crypto/sad-launcher/raw/main/site/download/SadLauncherSetup.exe`. Итоговый размер `erootstars` снизился до `16.25 МБ` (макс. файл `0.70 МБ`), все 36 тестов (`test_launcher_host`, `test_v138`, `test_v139`) зелёные.
- **Интеграция 7 компонентов 21st.dev и ReactBits на сайт (`site/index.html`, `site/styles.css`, `site/app.js`)**:
  - В чистом HTML/CSS/JS (без бандлеров, в чёрно-белой стеклянной эстетике с изумрудным акцентом `#82e6a5`) портированы все 7 присланных компонентов:
    1. **21st.dev `Spotlight` + `GridPatternSpotlight` + `Hero`**: SVG-паттерн `#grid-pattern` с курсорным прожектором `#hero-grid-spot`, нижней радиальной дугой горизонта `.hero__arc` и кликабельным бейджем `.eyebrow`.
    2. **ReactBits `<Aurora />`**: WebGL2-шейдер северного сияния (`#aurora-canvas`) с симплекс-шумом в серебристо-изумрудной гамме.
    3. **ReactBits `<Particles />`**: интерактивные частицы (`#particles-canvas`) с параллаксом, отталкиванием от курсора, мерцанием и волной при клике.
    4. **ReactBits `<BlurText />`**: пословное проявление заголовков из размытия (`[data-blur-text]`) + режим `data-focus-hover="true"` (наведение на слово фокусирует его и мягко размывает соседние).
    5. **ReactBits `<MagicBento />`**: интерактивная сетка из 6 карточек (`#magic-bento`) с общей подсветкой границ по близости курсора, 3D-наклоном, волной от клика, звёздной пылью (`.magic-bento__stars`) и живыми виджетами (интерактивный скраб-график скорости `#mb-chart`, тумблеры эффектов, комната с двумя курсорами и двумя адресами LAN/Интернет, 4-шаговый пайплайн и хаб загрузчиков).
    6. **ReactBits `<SpotlightCard />`**: карточки скачивания ОС (`#spotlight-cards`) с передачей света между соседними карточками (`proximity: 90px`), зерном SVG `fractalNoise` и вспышкой при нажатии.
    7. **ReactBits `<Dock />`**: плавающая нижняя панель разделов (`#dock`) с плавным увеличением иконок под курсором (`44px -> 62px`), подсказками и индикатором активной секции.
- **Автокликер в миллисекундах и Анти-АФК до 60 минут (`effects.rs`, `automation.rs`, `views/effects.rs`)**:
  - Скорость автокликера переведена в **миллисекунды (`мс`)**: поле `autoclicker_ms` (по умолчанию `10 мс` ≈ 100 кликов/с), ползунок `1..=250 мс` и быстрые пресеты `5 мс`, `10 мс (100/с)`, `25 мс`, `50 мс`, `100 мс`.
  - Максимальный интервал защиты от АФК увеличен с `300 с` до **`3600 с` (`60 мин`)** с форматированием минут и пресетами `30с`, `60с`, `5м`, `15м`, `30м`, `60м`.
- **Адаптивная сетка каталога модов без пустого поля справа и кликабельность всех кнопок (`main.rs`, `views/mods.rs`, `views/play.rs`, `views/server.rs`, `app.rs`)**:
  - Начальный размер окна лаунчера увеличен до `1240×760` (помещается 5 карточек в ряд), а функция `grid_cell_size(avail_w)` в `views/mods.rs` растягивает карточки равномерно на 100% доступной ширины строки в `catalog`, `skeleton_grid` и `hit_card`, полностью убирая пустую полосу справа.
  - Все кнопки в лаунчере сделаны кликабельными с понятной обратной связью; ссылка на сайт `https://rkt5jy.bot-keep.xyz` добавлена в описание окна «О лаунчере», шапку вкладки «Играть», на сайт и в описание GitHub-репозитория.



## Обновление 2026-11-17 (23:30) — аудит после слабой модели: скорость, загрузки, картинки, FAQ сайта

**Замеры (dev-хук `MADRID_BENCH=<сек>` + `MADRID_BENCH_OUT=<файл>`: непрерывная перерисовка, в файл «кадры мс_UI»; скрипты замера — PowerShell, CPU процесса / кадры).** Время UI на кадр до → после: «Эффекты» 1,29 → 0,91 мс (−29 %), «Настройки» 2,25 → 1,25 мс (−44 %, 155 → 179 кадров/с — пропали рывки), «Моды» CPU/кадр 2,59 → 2,36 мс, «Старт» без изменений (~1,6 мс). Память в простое до/после одинакова: ~80–85 МБ приватной (это в основном GL-драйвер + egui), рабочий набор ~10 МБ после trim. Регрессии RAM от кода слабой модели не найдено; найдено лишнее CPU/диск.

**Что было не так и исправлено:**
- `views/effects.rs`: `options.txt` читался и патчился с диска КАЖДЫЙ кадр (`inspect_instance_applied`) → кэш `effects_applied_state()` (раз в 1 с или при смене настроек). `is_effect_mod_installed` делал `to_lowercase()` на каждый мод × 6 эффектов × кадр → сравнение без аллокаций.
- Слайдеры «Эффектов» и «Настроек» писали `effects.json` / `config.json` на диск каждый кадр перетаскивания → флаги `effects_dirty` / `cfg_dirty`, запись при отпускании мыши (и в `on_exit`).
- `effects::apply_to_instance` при каждом запуске создавал ПУСТОЙ `options.txt` в сборке без него, а вкладка показывала «Нажми Применить» → пишет только если текст реально меняется.
- `automation.rs`: фоновый поток в простое дёргал GetForegroundWindow/GetWindowText 7–28 раз/с и EnumWindows каждые 0,5 с всегда → в простое (игра не запущена, АФК/кликер выключены) окна проверяются раз в 0,4 с, полный обход раз в 2,5 с. Автокликер ловил ЛЮБОЕ GLFW-окно (другие игры!) → чужое окно считается Minecraft только с «minecraft» в заголовке (или LWJGL / PID нашей игры). Кликер: `timeBeginPeriod(1)` только пока кликает (иначе Windows спит шагами 15,6 мс и «10 мс» были ~16 мс), ровный шаг без дрейфа.
- `icons.rs`: SVG-иконки рисовались отрезком + кругом на КАЖДУЮ точку кривой Безье (~10× лишних фигур на тесселяцию каждый кадр) → один `Shape::line`/`closed_line` на контур, круглые концы только на концах.
- Настройки: размер хранилища считался обходом всей папки данных в UI-потоке раз в 4 с → `spawn_blocking`, UI показывает последнее значение.
- Список модов: `mods::installed` каждую секунду читал папку + 2 stat на jar → перечитывается только при смене mtime папки (или раз в 30 с), `entry.metadata()` без лишнего stat, `sort_by_cached_key`. `profiles::list_profile_mods` клонировал весь список каждый кадр ради `.len()` → `Arc<Vec<_>>`.
- Каталог: бейдж карточки показывал новейшую версию проекта («26.3 · Fabric») при сборке 1.20.1 Forge → показывает версию/загрузчик активной сборки, если проект их поддерживает (Modrinth и CF).

**Загрузки (`net.rs`, `minecraft.rs`):** части большого файла (до 8 диапазонов) шли через общий HTTP/2-клиент = ОДНО TCP-соединение на хост → `net::bulk_client()` (HTTP/1.1, каждая часть — своё соединение, как у менеджеров загрузок; только на общем рантайме). Основной клиент: `http2_adaptive_window(true)`. Живой тест `cargo test --release live_range_speed -- --ignored --nocapture` (client.jar 1.20.1, 22 МБ): канал владельца ~4–5 МБ/с упирается сам, разброс большой (h2 2,7/4,6 МБ/с, h1 4,7/4,0 МБ/с) — выигрыш виден на линиях быстрее/с потерями. Прогресс ассетов теперь идёт до конца очереди (раньше замирал на последних ~128 файлах).

**Картинки лаунчера:** семафор (16 вместо 10) держится только на сетевую часть — иконки с диска больше не ждут в очереди за скачиваемыми; при открытом каталоге иконки следующих ~5 рядов грузятся сразу после ответа поиска (на других вкладках — нет: стартовый поиск не тратит RAM/CPU); галерея 640 → 480 px (плитка 240 × 2); `Bytes → Vec` без копии.

**Лаунчер:** кнопка «Сайт» внизу боковой панели (рядом с «О лаунчере»). Dev-скриншоты: `MADRID_DEV_ZOOM=2 MADRID_DEV_SHOT=<file.png> [MADRID_DEV_SHOT_DELAY=8]` — egui `request_screenshot` (работает с прозрачным GL-окном, в отличие от PrintWindow), ГРАБЛИ: без непрерывного `request_repaint()` фоновое окно не рисует кадр, и колбэк скриншота не вызывается.

**Сайт:** скриншот каталога заменён на чёткий 2× (`assets/shot-mods@2x.webp` 140 КБ + `.jpg` 260 КБ, `<picture>` + srcset); ГРАБЛИ: у `.tilt__frame` был `will-change: transform` при стартовом `scale(0.96) rotateX(9deg)` — Chrome растрировал скриншот в уменьшенном масштабе и растягивал (мыло) → will-change убран. Bento «Что внутри»: площадь карточек 16 ячеек при сетке 6×3 давала дыру справа → «Загрузчики» на 4 колонки (текст слева, схема справа), пайплайн заполняет высоту, ряд 262 px (список «Эффектов» не обрезается). Заголовок hero больше не размывает соседние слова при наведении. FAQ — порт 21st.dev Slit Type FAQ на чистом JS в одной «чернильной» палитре (светлые чернила на тёмном): буквы заголовка поднимаются сквозь прорезь, строка заливается слева с инверсией текста, один открытый вопрос, ↑↓/Home/End, `#faq-*` открывает вопрос (ссылки из bento), «Скопировать ссылку», поиск с подсветкой и фрагментами ответа, темы-чипы со счётчиками, пустое состояние, «Спросить на GitHub» (issues). Токен `--dim` поднят до #8f8f99 (контраст). CSS/JS подключаются с `?v=` — после выкладки у посетителей не останется старый кэш (локально http.server отдавал старый styles.css). Удалены неиспользуемые shot-build.png / shot-mods.png.

## Обновление 2026-11-18 — автообновление проверено, ролики Remotion, мобильная версия сайта

**Автообновление (проверено сквозным живым тестом).** Цепочка: лаунчер при старте спрашивает `api.github.com/.../releases/latest` (релизов нет → 404) → берёт `raw.githubusercontent.com/.../main/site/version.json` → если версия новее, карточка «Обновиться» → установщик качается с `rkt5jy.bot-keep.xyz/download/SadLauncherSetup.exe` (302 на github raw) → запускается с `--update <exe>`, ждёт закрытия, заменяет только exe и запускает его. Тест: `cargo test --release live_update_e2e -- --ignored --nocapture` (притворяется 0.0.1, качает настоящий установщик, тот заменяет фиктивный exe в %TEMP%, перезапущенный лаунчер делает скриншот через MADRID_DEV_SHOT) — прошёл: 18,6 МБ установщик, exe заменён (13,2 МБ), новый лаунчер отрисовал окно.
- **Как выпустить обновление пользователям:** поднять `version` в `Cargo.toml` → `bash scripts/make-dist.sh` (сам пишет `site/version.json` и кладёт `site/download/SadLauncherSetup.exe`) → commit + push в main. Через ~5 мин (кэш raw) у всех лаунчеров при запуске (и раз в 6 часов у открытых) появится карточка.
- Исправлено: `config::VERSION` теперь `env!("CARGO_PKG_VERSION")` (была вторая ручная копия — поднял одну и забыл другую = вечное «доступно обновление»); у скачивания установщика убран общий таймаут 180 с (на канале < 100 КБ/с обновление всегда обрывалось); повторная проверка раз в 6 часов; перед обновлением лаунчер закрывается штатно (раньше `process::exit(0)` оставлял работать Java-сервер и проброшенный UPnP-порт), установщик ждёт до 60 с вместо 24.
- Заготовка `.github/workflows/release.yml` (тег `v*`) собирает артефакты, но GitHub Release не создаёт → путь через releases/latest пока не используется, работает version.json.
- ГРАБЛИ: скрипты `install-macos.command` / `install-linux.sh` ищут сборку в releases/latest, а её нет → ставят Rust и собирают из исходников (а код лаунчера Windows-ориентирован). Подписи на сайте смягчены; нужен настоящий релиз с mac/linux-артефактами или честное «скоро».

**Сайт — карточки «Что внутри».** Было: 6 мини-виджетов с мелким текстом, звёздной пылью, обрезанными списками и наложениями на узкой ширине. Стало: ровная сетка 3×2 (на ≤960 px — 2 колонки, на ≤600 — одна), в каждой — короткий ролик + заголовок в 2–3 слова + одна строка. Ролики — проект Remotion в `video/` (`src/cards/*.tsx`, `bash video/render-cards.sh`): Speed, Effects, Friend, Pipeline, Search, Loaders, 960×540, 5–6 с, бесшовная петля (затухание на стыке). Форматы: WebM VP9 с альфа-каналом (90–140 КБ, Chrome/Firefox) + MP4 на чёрном с `mix-blend-mode: screen` (Safari) + прозрачный PNG-постер. Видео грузятся и играют только на экране (IntersectionObserver), при reduced-motion/save-data — постер; если автозапуск запрещён (iOS Low Power) — перемотка на кадр с содержимым (первый кадр петли пустой). ГРАБЛИ: `mix-blend-mode` на `<video>` в фоновом/программном рендере не работает → основной формат — прозрачный WebM, а не blend.

**Сайт — телефоны.** Убраны синие квадраты при нажатии (`-webkit-tap-highlight-color: transparent`) и рамки фокуса после клика мышью; фокус с клавиатуры остаётся, но белый (`:focus-visible`), без синего. Зоны нажатия ≥ 44 px на сенсорных экранах, кнопки hero на телефоне одинаковые на всю ширину (327×50), бейдж в одну строку. Текст сокращён по всему сайту: hero (бейдж, подзаголовок, 2 кнопки вместо 3), подписи секций, статистика, карточки скачивания. Проверено на 375/640/1440 px: без горизонтальной прокрутки, консоль чистая.

## Обновление 2026-11-18 — правило выпуска, v0.7.1

- Новое правило (AGENTS.md, allskill, ANTIGRAVITY.md): после правок `src/` агент сам поднимает patch-версию в `Cargo.toml`, запускает `make-dist.sh` и пушит в main — пользователи получают «Обновиться». Только сайт/доки — без бампа.
- Выпущена 0.7.1: все оптимизации и исправления обновлятора этой сессии.

## Обновление 2026-11-18 — сайт на Botkeep

- Botkeep берёт сайт из репозитория `erootstars` (`launcher_site/`), а не из `sad-launcher`. Сайт перенесён туда (коммит dc877ad), `.webm` добавлен в долгий кэш. Правило синхронизации — в AGENTS.md, п. 8. В панели Botkeep выключено «Pull before Start/Restart» — после пуша нужно нажать синхронизацию/перезапуск вручную (или включить галочку).

## Обновление 2026-11-18 (ночь) — v0.7.2: macOS и Linux по-настоящему, «Проверить обновления», ролики 1080p

**macOS / Linux.** CI (`.github/workflows/release.yml`) собирает и тестирует лаунчер на ubuntu-22.04 и macos-latest — обе сборки зелёные (до этого код под unix ни разу не компилировался). По тегу `v*` публикуется GitHub Release: `SadLauncher-macos-universal.tar.gz` (Universal: arm64+x86_64 через lipo, иконка .icns, Info.plist с версией из Cargo.toml, ad-hoc `codesign` — без подписи Apple Silicon не запускает arm64-код) и `sad-launcher-linux-x86_64.tar.gz` (бинарник + иконка + .desktop). Windows-установщик в релиз НЕ кладётся: Windows обновляется по `site/version.json`, как раньше. Ручной запуск workflow = только сборка, без публикации.
- Самообновление на macOS/Linux (`updater.rs`): ассет релиза под свою платформу (`ASSET_NAME`), проверка gzip-заголовка, `tar -xzf` во временную папку, `rename` поверх работающего бинарника, перезапуск с `MADRID_RELAUNCH_WAIT=1` (новая копия ждёт до 60 с, пока старая отпустит `instance.lock`). `version.json` на unix не читается (там Windows exe).
- macOS: `-XstartOnFirstThread` для версий до 1.13 (без него LWJGL не открывает окно); на Apple Silicon запасная x64-Java через Rosetta (у Temurin нет arm64 Java 8).
- Вкладка «Эффекты» на macOS/Linux: вместо неработающих автокликера/АФК (Win32 API) — плашка «пока только на Windows».
- Скрипты `site/download/install-*.{sh,command}` берут сборку из releases/latest; Linux подсказывает недостающие библиотеки (apt/dnf/pacman) и PATH. Сайт: на macOS — команда для Терминала (у скачанного браузером .command нет +x, Gatekeeper блокирует) и прямая ссылка на .app; на Linux — команда + архив.
- ГРАБЛИ: раннер `macos-14` недоступен для аккаунта («configuration not supported») → `macos-latest`. Локально проверить unix-сборку нельзя: aws-lc-sys/zstd-sys требуют кросс-компилятор C, WSL нет — только CI.

**«Проверить обновления»** — строка «Версия X · статус» + кнопка в «О лаунчере» и внизу «Настроек». `updater::probe` различает «есть новая» / «это последняя» / «нет связи» (раньше офлайн выглядел как «обновлений нет»). Ручная проверка показывает даже версию, скрытую кнопкой «Не показывать»; при найденной версии кнопка становится «Обновиться».

**Скорость загрузки.** Лимитов в лаунчере нет: 71 Мбит/с = ~8,9 МБ/с, лаунчер показывает МБ/с; замер (`cargo test --release live_small_files_speed -- --ignored --nocapture`, MADRID_CONC=…) — крупные файлы ~7 МБ/с, 600 мелких ассетов ~5–6 МБ/с; HTTP/1.1-пул против одного HTTP/2-соединения разницы не дал; 128 параллельных ассетов вместо 64 — +16 % (277 → 323 файла/с), 192 — без прироста → поставлено 128. Провалы на счётчике — этапы без сети (установщик Forge, распаковка, проверка Java).

**Ролики карточек** перерендерены в 1920×1080 (`--scale=2`), VP9 crf 20 / H.264 crf 18: WebM 320–490 КБ, всего 3,6 МБ. «Пайплайн» отцентрирован.

## Обновление 2026-11-18 — Botkeep: превышен лимит 20 МБ

- Импорт Botkeep (1000 файлов, 4 МБ на файл, 20 МБ всего) остановился: ролики и скриншот (~4 МБ) довели `erootstars` до 20,9 МБ. Решение: в `launcher_site/` только HTML/CSS/JS/скрипты, медиа (`assets/cards/*`, `shot-mods@2x.*`) отдаются jsDelivr из `sad-launcher@main` (типы video/webm, video/mp4 верные); в `LAUNCHER_CSP` добавлен `media-src https://cdn.jsdelivr.net`. Репозиторий 16,8 МБ, 518 файлов. Правило синхронизации обновлено в AGENTS.md п. 8. jsDelivr кэширует @main до ~12 ч — новые ролики появляются с задержкой (или менять имя файла).
- В тестах магазина `test_v98_rich::test_qr_goes_as_separate_photo_when_header_failed` падает и без этих правок — старая проблема, к сайту не относится.

## Обновление 2026-11-20 — v0.7.3: установка в 2 раза быстрее, остатки переписаны на Rust, скилл rust-anything

**Скорость установки (замер `cargo test --release live_install_timing -- --ignored --nocapture`, чистая папка, Forge 1.20.1, 713 МБ; `MADRID_GAME=<папка>` оставляет её, `MADRID_LOADER=forge|fabric|neoforge|vanilla`, `MADRID_MC=…`).** До: 125–160 с, после: 70 с.
Причины «быстро в начале — медленно в конце» (по замеру скорости каждые 5 с):
1. Установщик Forge (Java) стартовал только ПОСЛЕ последнего ресурса и 31,6 с работал без прогресса: ~10 с качал свои библиотеки по одной, остальное — процессоры (jarsplitter, ART, binarypatcher, CPU). Теперь `install()` запускает ресурсы и ветку «Java + загрузчик» параллельно (`tokio::try_join!`, события сводит `Funnel`: полоса — ресурсы, хвост подписи «+ Forge»). Перед установщиком `prefetch_installer_libraries` качает библиотеки из install_profile.json + version.json внутри jar в 32 потока, и установщик запускается с `--offline` (при ошибке — повтор без него). Только процессоры: 22,5 → 19,2 с, и они скрыты за ресурсами.
2. Ресурсы шли в случайном порядке → в конце оставались крупные музыкальные файлы на 2–3 соединениях (1,7–3 МБ/с). Теперь `asset_plan`: крупные первыми, дубликаты хэшей убраны (раньше два задания могли писать один `.part`), прогресс по байтам (размеры есть в индексе), а не по числу файлов.
3. Все 128 запросов мелких файлов шли по ОДНОМУ HTTP/2-соединению (окно одного TCP): 4–8 МБ/с при 17 МБ/с у крупных файлов на нескольких соединениях. `net::small_file_client(i)` — 12 клиентов (своё соединение у каждого), задания по кругу: ресурсы 65 → 52 с (замер 1 / 6 / 12 соединений: 65,2 / 54,8 / 51,8 с). Дополнительные Range-соединения ограничены общим семафором `net::SEGMENTS` (48) — иначе при «крупные первыми» каждый из 128 файлов открывал до 7 соединений.
- ГРАБЛИ: `--offline` у установщика Forge не отключает задачу DOWNLOAD_MOJMAPS (installertools качает маппинги сам, ~3 с). Канал владельца на крупных файлах держит до 17 МБ/с — «71 Мбит» из прошлых замеров был потолком одного соединения.

**Переписано на Rust (`src/win.rs`, общий с установщиком через `#[path]`, без новых крейтов):**
- Ярлыки (лаунчер и установщик): PowerShell `WScript.Shell` → COM `IShellLinkW` + `IPersistFile` в отдельном потоке (COM не смешивается с UI-потоком, где rfd тоже использует COM).
- Реестр: `reg.exe` (автозапуск, перенос старого имени, выбор дискретной видеокарты при КАЖДОМ запуске игры) → `RegSetValueExW` / `RegDeleteKeyValueW` / `RegGetValueW`.
- Домашний IP для игры с другом: разбор локализованного вывода `route print` → `GetIpForwardTable` + `GetIpAddrTable` (`upnp::pick_home_ip`, на Vista+ metric1 уже включает метрику интерфейса).
- Поиск Java: `where java` / `which -a` → обход PATH (`java::java_on_path`).
- `scripts/make-dist.sh` → крейт `xtask` (workspace): `cargo run -p xtask -- dist`; sh-файл оставлен однострочной обёрткой, чтобы команды в документах не менялись.
- Удалены 35 одноразовых `scripts/patch-*.py` и `mods-view-body.txt` (давно применённые патчи; история — в git).
- Осталось не на Rust намеренно: `.bat`-петли самоудаления exe при удалении/обновлении (запущенный exe не может удалить сам себя; нужен процесс, который переживёт лаунчер), `tar` для Java/обновления на macOS/Linux, сайт (`site/`), ролики Remotion (`video/`, React по природе), `setup-antigravity.ps1` (настройка агента, не лаунчер).

**Скилл `rust-anything`** (github.com/erkamyaman/rust-anything, MIT) установлен в `~/.claude/skills/rust-anything` (+ копия в `antigravity/skills/`), вписан в allskill (шаг 2, «Перенос кода на Rust») здесь, в `D:\working WIN\eroot-store\.claude\skills\allskils\SKILL.md` и `D:\working WIN\Claud-vpn 1\.claude\skills\allskills\SKILL.md`. Его CLI (`cargo install rust-anything`) не установлен: автоматическая установка заблокирована, ставить владельцу вручную.

## Обновление 2026-11-21 — v0.7.4: живые данные каталога, версия сборки и «Обновить сборку», видео, без «□» в описаниях

**Скачивание сборки (замер `MADRID_PACK=cobbleverse cargo test --release live_pack_speed -- --ignored --nocapture`, теперь с графиком МБ/с и числа файлов каждые 2 с).** Cobbleverse: архив 230 МБ за 37 с, 166 файлов / 697 МБ за 98–100 с, в среднем 7–9 МБ/с. Канал владельца в момент замера — потолок ~5–8 МБ/с: curl одним и шестью соединениями к Modrinth, Mojang и speed.cloudflare.com дал те же 4–6 МБ/с, а 16 Range-соединений на архив не ускорили его. Значит, сама загрузка упирается в линию. «Замедление в конце» было ощущением от полосы: она считала ФАЙЛЫ, а в паке есть ресурспаки на 133/118/88 МБ, которые в случайном порядке оставались последними — полоса стояла на 95 % и ползла. Исправлено:
- файлы пака качаются от крупных к мелким (`fetch_pack_files`, размеры — из индекса mrpack и пакетного ответа CF);
- полоса идёт по БАЙТАМ (новое событие `ModEvent::Bytes`, живой счётчик принятых байт каждого файла; счётчик «5/120» по-прежнему считает файлы);
- первый запрос каждого файла (`net::fetch_to`) крутится по 12 HTTP/2-соединениям (раньше это делали только ресурсы); огромные файлы (≥48 МБ) делятся на 16 частей вместо 8; общий лимит Range-соединений 64.

**Последние версии сборок.** Проверено живьём: и Modrinth (`/project/{id}/version`), и CurseForge (`/mods/{id}/files?pageSize=1`) отдают при установке действительно последнюю версию пака (тест `live_pack_versions`). Просьба «обновиться» в игре шла не от пака, а от Forge VersionChecker и модов (в логе: `[forge] OUTDATED Current: 47.4.10 Target: 47.4.28`, `Collective has an update available`): автор пака закрепляет версии Forge и модов, а Forge сообщает о новых. Теперь в сборках не «Мадрид» на Forge/NeoForge лаунчер перед запуском ставит `config/fml.toml: versionCheck = false` (`launch::quiet_version_check`, меняет только этот ключ): нет ложной просьбы обновиться и ~100 лишних HTTP-запросов при загрузке игры.
- Профиль пака помнит, какая версия стоит: `pack_file` (id версии Modrinth / файла CF), `pack_version`, `pack_date` (`serde(default)`, старые профили читаются). `mods::latest_pack_file` — один источник «последней версии» для установки и проверки; на Modrinth берётся самая новая по дате, а не первая в списке.
- Вкладка «Сборка», пак: «Версия сборки: 1.8.0 · от 17 нояб. 2026», затем фоновая проверка (`Launcher::pack_check`, `app::PackCheck`): «последняя версия» (зелёный) или «Вышла версия X · дата (N дн. назад)» + кнопка «Обновить сборку» (переустановка в ту же сборку, миры/скриншоты/настройки остаются). Паки, поставленные до 0.7.4, показывают «Версия сборки неизвестна» и предлагают обновиться — после переустановки версия запоминается.

**Каталог — живые цифры.** На плитке: число скачиваний и у модпаков (раньше вместо него было только «N модов · размер»), пилюля «обновлено N дн. назад» в правом верхнем углу картинки (`ModHit.updated`: Modrinth `date_modified`, CF `dateReleased`). Открытая карточка запрашивает проект напрямую и подменяет скачивания/дату в сетке и кэше страниц (`apply_fresh_detail`); кэш страниц поиска 10 → 3 мин. В карточке: «обновлено 17 нояб. 2026 (3 дн. назад)», даты у версий. Даты: `util::parse_iso_unix` / `ru_date`, `mods::ago`.

**Видео в карточке.** Секция «ВИДЕО»: id YouTube вытаскиваются из описания (embed/nocookie/watch/youtu.be/shorts — `mods::youtube_ids`), превью `i.ytimg.com/vi/ID/hqdefault.jpg` через общий кэш картинок, стеклянная кнопка ▶, клик открывает YouTube (встроенный плеер в egui — десятки МБ, не стоит). Нет видео в описании — кнопка «Найти видео на YouTube» (поиск «<название> minecraft modpack»). У ATM10 найдено 4 видео, у BMC2 — 4.

**«□» в описаниях.** Эмодзи и значки, которых нет в шрифтах (✅, ⭐, значки из private use, ZWJ/VS16), egui рисовал пустыми квадратами. `app::drawable_text` выкидывает символы, для которых `fonts.has_glyph` = false, и пробел, оставшийся в начале строки; применяется к описанию и подзаголовку карточки один раз при получении.

**Отладка:** `MADRID_DEV_DETAIL=1` (+ `MADRID_DEV_TAB=modpacks MADRID_DEV_QUERY=…`) открывает карточку первого результата — для скриншотов.

**Скиллы:** в allskill (здесь, `eroot-store`, `Claud-vpn 1`) добавлен шаг «Нет скилла — скачай все сразу» с командами для Git Bash и PowerShell: sparse-clone публичного `sad-launcher` (папки `antigravity/skills` и `antigravity/commands`), копируются только отсутствующие скиллы и команды. В пакет добавлены недостающие 16 скиллов (ub-mode, pricing, signup, remotion-docs, ecc-backend-patterns, caveman-*) и 20 команд (`/tov`, `/ui-audit`, `/live`, `/apple-hig`, `ecc-*`). CLI rust-anything владелец поставил сам.

## Обновление 2026-11-21 (ночь) — v0.7.5: YouTube прямо в лаунчере, просмотрщик как в Telegram, колесо вбок, автоперевод описаний

**Видео в лаунчере (`src/video.rs`, Windows).** Официальный плеер YouTube (IFrame API, `youtube-nocookie.com`) в дочернем окне WebView2 (`wry` 0.57) поверх области видео в просмотрщике. Это легально (без вытаскивания потоков) и быстро: движок Edge из Windows 11, аппаратное декодирование, качество до 4K. Окно создаётся скрытым сразу при открытии карточки с видео, первое видео уже грузится, поэтому клик запускает его сразу. Скрывается и ставится на паузу при закрытии просмотрщика, удаляется вместе с карточкой (живёт ~100 МБ). Страница плеера отдаётся своим протоколом `sad://player/` (на Windows это `http://sad.player/`): у встроенного плеера появляется origin/referrer, иначе YouTube отказывает («Error 153»). События плеера (ready/playing/error) приходят через `window.ipc` → `VideoState`. Ошибки 101/150 («автор запретил встраивание») показываются текстом, и есть кнопка «Открыть на YouTube». На macOS/Linux видео открывается в браузере. Данные WebView2 лежат в `%APPDATA%\Sad Launcher\webview`. Проверено живьём (`MADRID_DEV_DETAIL=1 MADRID_DEV_VIDEO=1`): в журнале «Видео tLTC6wYcWVM играет в лаунчере» через ~5 с после старта; кадр видео снят через PrintWindow(PW_RENDERFULLCONTENT) — egui-скриншот дочернее окно не захватывает.
- ГРАБЛИ: на тулчейне GNU `webview2-com-sys` линкует `WebView2Loader.dll` через таблицу импорта — без DLL рядом exe вообще не запускается. Решение: `vendor/webview2-com-sys` (через `[patch.crates-io]`): функции загрузчика берутся во время работы (LoadLibrary/GetProcAddress) из официального `WebView2Loader.dll` (160 КБ, вшит в exe, при первом видео пишется в `%LOCALAPPDATA%\Sad Launcher\WebView2Loader-160880.dll`). В импортах exe его нет (проверено `objdump -p`). Сборки MSVC оставлены как в оригинале.

**Просмотрщик (`views/viewer.rs`).** Как в Telegram: окно затемняется, видео или скриншот по центру во весь доступный размер (16:9 или пропорции картинки), круглые стеклянные стрелки по бокам (и ← / →), снизу слева «название · Видео 2 из 5», справа «Открыть на YouTube», сверху справа «Закрыть»; Esc, клик по тёмному полю или боковая кнопка мыши «назад» закрывают просмотрщик. Видео и скриншоты карточки — один список (сначала видео). Картинки грузятся до 1920 px.

**Колесо вбок.** Над лентой видео или скриншотов вертикальная прокрутка колесом превращается в горизонтальную (`wheel_scrolls_sideways`): листается лента, страница не уезжает вниз.

**«Перевести на русский» (`src/translate.rs`).** Заметная кнопка прямо над описанием любого проекта (мода, модпака и т.д.), только там. Нет её, если описание уже на русском (≥40 % кириллицы). Перевод — публичный веб-эндпоинт Google Translate (`client=gtx`, без ключа): куски ≤4500 символов по абзацам параллельно, склейка по порядку, кэш на сессию. После перевода — «Показать оригинал» и подпись «переведено автоматически · Google»; ошибка — красным, с кнопкой «Перевести ещё раз». Живой тест: `cargo test --bin sad-launcher live_translate -- --ignored --nocapture`.

**Defender.** В 04:36 Windows Defender удалил `assets/launcher-payload.exe` (бинарник 0.7.4) как `Trojan:Win32/Bearfoos.A!ml`: эвристика машинного обучения сработала ложно на неподписанный exe (автозапуск в реестре, самообновление, bat самоудаления). Новые 0.7.5 (`Sad Launcher.exe`, установщик, payload) по `Start-MpScan` чистые. Если повторится — отправить файл в Microsoft как ложное срабатывание (microsoft.com/wdsi/filesubmission); надёжное решение — подпись кода.

## Обновление 2026-11-21 — v0.7.6: одна лента медиа, колесо вбок только над лентой, без «□» в установщике

- **Колесо вбок было на всю страницу.** У `ScrollArea::horizontal` в egui прямоугольник обрезки по вертикали не ограничен (берётся у родителя), поэтому проверка «курсор над лентой» через `ui.clip_rect()` срабатывала по всей высоте карточки: страница не листалась вниз, оставался только ползунок справа. Теперь прямоугольник ленты считается ДО отрисовки (`cursor().min` × ширина × высота плитки), и вбок колесо работает только над ним (`wheel_sideways_over`) и только если лента шире карточки. Если всё помещается, колесо листает страницу и над лентой.
- **Видео и скриншоты — одна строка** (`draw_media_strip`, плитка 224×126): сначала видео с ▶, потом скриншоты, заголовок «ВИДЕО И СКРИНШОТЫ» / «ВИДЕО» / «СКРИНШОТЫ». Без видео в описании кнопка «Найти видео на YouTube» стоит рядом с заголовком. Под лентой сразу видно описание: страницу можно листать колесом с любого места ниже.
- Прокрутка карточки сохраняется отдельно для каждого проекта (`id_salt` с source+id): новая карточка открывается сверху, а не там, где листали прошлую (egui сохраняет память между запусками).
- **Установщик:** «✓» и «⚠️» рисовались пустыми квадратами (в шрифтах нет глифов) — галочка теперь нарисована (кольцо + штрих, зелёный), у предупреждения знак убран.
- Defender: 0.7.6 (лаунчер, установщик, payload) по `Start-MpScan` чистый.

## Обновление 2026-11-21 — v0.7.7: эффекты меняются прямо в игре, набор FPS-ресурспаков

**Живые эффекты.** Ресурспак эффектов стал папкой `resourcepacks/SadEffects/` (раньше `SadEffects.zip`) и включён в сборке ВСЕГДА (`file/SadEffects` в options.txt; старый zip и его запись удаляются). Переключатель эффекта → `apply_to_instance` переписывает только изменившиеся файлы пака (лишние удаляет, `write_pack_dir`) → если игра запущена, `automation::reload_game_resources` находит окно Minecraft и шлёт ему F3+T через PostMessage (скан-коды в lParam, GLFW распознаёт клавиши по ним) — игра перечитывает ресурсы, фокус и нажатия игрока не нужны. Статус: «Применено в игре · ресурсы перезагружены». Игра, запущенная до 0.7.7, без пака в списке: нужен один перезапуск.
- **Яркость и туман теперь тоже на лету** (1.17+): `shaders/include/light.glsl` и `fog.glsl` берутся из клиентского jar ЭТОЙ версии и патчатся, а не пишутся заново: исходная функция переименовывается в `sad_orig_*`, добавляется обёртка — `minecraft_sample_lightmap` тянется к белому на силу ползунка (Fullbright), `linear_fog`/`linear_fog_fade` возвращают цвет без тумана или вызывают оригинал с `fogStart/fogEnd × 1/плотность`. Нет функции в шейдере → эффект просто не добавляется. Работает в ванильном рендере; Sodium/Embeddium/Iris рисуют ландшафт своими шейдерами — для них остаются моды (кнопка «Скачать мод»). Тест на настоящих шейдерах 1.20.1: `real_client_shaders_patch`.
- Ещё убирается виньетка (вместе с тыквой).
- Гамма в options.txt, тряска камеры, частицы (настройка), молнии — по-прежнему после перезапуска (Minecraft читает options.txt только при старте и перезаписывает его при выходе).

**Набор «FPS и чистый экран»** (`effects::RP_SET`, вкладка «Эффекты»): самые скачиваемые паки Modrinth без модов — Low Fire (8 млн), Low Shield (5,9 млн), Clear Glass, Fast Better Grass (14 млн), Even Better Enchants, Better Lanterns. Паки, которым нужны моды (Fresh Animations → EMF/ETF), и паки, режущие FPS (Better Leaves), не взяты. Скачиваются один раз в `meta/rp-cache/<slug>-<mc>.zip` (версия под MC сборки или новейшая) и ВЛИВАЮТСЯ в папку SadEffects (только `assets/…`, без `..`, ≤64 МБ): поэтому тоже включаются/выключаются прямо в игре. Свои текстуры лаунчера перекрывают текстуры набора.

**Не проверено в живой игре**: владелец в этот момент играл в Roblox — Minecraft и окна лаунчера для скриншотов не запускались. Код покрыт тестами (пак, папка, патч шейдеров, опции); F3+T через PostMessage и компиляцию пропатченных шейдеров надо проверить запуском (`MADRID_FULL_BOOT=1 cargo test live_forge -- --ignored`, затем переключить эффект и смотреть в game-*.log «Reloading ResourceManager» без ошибок шейдеров).

## Обновление 2026-11-21 — SEO сайта и защита (без версии лаунчера)

- **Почему сайта нет в Google:** nginx Botkeep отдаёт `X-Robots-Tag: noindex, nofollow` на `rkt5jy.bot-keep.xyz` (и на eroot.shop) — это не наш код, из репозитория убрать нельзя. Пока заголовок есть, Google сайт не проиндексирует, как бы ни была сделана разметка. Нужно: выключить в панели Botkeep (если есть настройка) или свой домен / другой хостинг (GitHub Pages из `site/` — индексируется, HTTPS бесплатно).
- На сайте: `robots.txt`, `sitemap.xml`, JSON-LD (Organization с founder, WebSite, SoftwareApplication), `meta robots`, Open Graph/Twitter-карточка, `humans.txt`, `.well-known/security.txt`. Синхронизировано в `erootstars/launcher_site` (медиа через jsDelivr), `version.json` там был 0.7.2 → 0.7.7.
- Шифрование: HTTPS/TLS даёт nginx Botkeep (HTTP → 308 на HTTPS). В `services/security.py` для сайта лаунчера HSTS 2 года и `upgrade-insecure-requests` в CSP.
- Тесты `test_launcher_host`, `test_v138`, `test_v139` — 36 зелёных; репозиторий 518 файлов, 16,05 МБ, макс. файл 0,7 МБ (лимиты Botkeep соблюдены).
- «Сертификаты основателя» купить нельзя за пользователя: подпись кода (Azure Trusted Signing, SignPath для open source, Certum) оформляет владелец; она же лечит ложные срабатывания Defender. Для Google «основатель» = разметка Organization + Search Console (подтверждение владения).
- **GitHub Pages включён** (Settings → Pages → GitHub Actions): https://simsislam89-crypto.github.io/sad-launcher/ — индексируется Google (без noindex). `pages.yml` публикует `site/` при каждом пуше в `site/**`, в копии подменяя адрес bot-keep на адрес Pages (canonical, sitemap). Botkeep-сайт остаётся как был.

## Обновление 2026-11-21 — v0.7.8: эффекты правда меняются в игре (26.x), автокликер кликает в окно, а не курсором

**Почему эффекты не включались в реальном времени (найдено по логу `game-20261121-064914.log`):** `Removed resource pack file/SadEffects from options because it is no longer compatible`. Игра владельца — Minecraft 26.3 (календарные версии), а `pack_format_for_mc("26.3")` проваливался в `_ => 3`, плюс с 1.21.9 (формат 65+) игра читает только `min_format`/`max_format`, а старый `pack_format` + `supported_formats` считает несовместимым и при старте выкидывает пак из options.txt. F3+T перезагружал ресурсы, но уже без пака.
- Формат пака теперь берётся из самого клиента (`version.json` в jar: `pack_version` числом, `{resource}` или `{resource_major}` — у 26.3 это 97) → `effects::client_pack_format`; таблица — только запасной путь (1.21.5 → 55 … 1.21.11 → 75, 26.x → «любой формат новой схемы»).
- `effects::pack_mcmeta`: формат ≥ 65 → `{"min_format":N,"max_format":N}` без `pack_format` (так же, как встроенные паки в jar 26.3), старые версии — как раньше.
- Страховка: `file/SadEffects` добавляется в `incompatibleResourcePacks` — игра оставляет такой пак включённым даже при несовпадении формата (как после «Всё равно включить»), а совместимый сама убирает из этого списка.
- **Шейдеры 26.x другие:** яркость — `vec4 sample_lightmap(...)` в `include/sample_lightmap.glsl` (в `light.glsl` её больше нет), туман — `float linear_fog_value(...)` в `fog.glsl` (через неё идут все туманы: окружение, дальность, небо, облака). Патч пробует старые имена, потом новые (`LIGHT_SHADERS`), выключенный туман = `return 0.0;`.
- **F3+T игнорировался при открытом меню паузы.** Байткод `KeyboardHandler` 26.3 (без обфускации) ссылается на `PauseScreen.showsPauseMenu`: debug-клавиши работают только без экрана. А меню паузы открывается само, как только игрок уходит в лаунчер (`pauseOnLostFocus`). Теперь, если игра не в фокусе и пауза включена, перед F3+T в окно уходит Esc (закрывает меню, на следующем кадре игра сама снова его открывает). Все сообщения идут одной пачкой без пауз — GLFW разбирает очередь за один опрос, меню не успевает вернуться. В 26.x клавиши переназначаемые (`key_key.debug.modifier`, `key_key.debug.reloadResourcePacks`) — берутся из options.txt (`automation::parse_game_options`).

**Автокликер кликает в окно игры, а не курсором.** Раньше `SendInput` жал настоящую кнопку мыши там, где курсор: в оконном режиме при открытом инвентаре/меню клик мог попасть на крестик окна и закрыть Minecraft. Теперь `WM_LBUTTONDOWN/UP` (ПКМ — `WM_RBUTTON*`) уходят сообщением прямо в HWND игры (`platform::send_mouse_click(hwnd, rmb)`), в другое окно или на заголовок клик попасть не может. В фоне кликает только в игру, запущенную лаунчером, и только если в её options.txt `pauseOnLostFocus:false` (F3+P): иначе меню паузы поймало бы клики (а там «Отключиться»). Плашка: «Кликает в фоне · кликов: N» / «Пауза: игра не в фокусе · для кликов в фоне нажми в игре F3+P».

**Не проверено в живой игре:** владелец снова был в Roblox — Minecraft и окно лаунчера не запускались (украли бы фокус). Проверено тестами на настоящем jar 26.3 (`real_26_client_pack_and_shaders`: формат 97, оба шейдера патчатся, mcmeta с `min_format`) и 1.20.1. Проверить при следующей игре: в game-*.log не должно быть `Removed resource pack file/SadEffects`, после переключения эффекта — `Reloading ResourceManager: vanilla, file/SadEffects`; автокликер с открытым инвентарём в оконном режиме не трогает рамку окна.

## Обновление 2026-11-21 — v0.7.9: живые тесты с запущенным Minecraft 26.3 нашли ещё 4 бага

Владелец попросил тесты с запущенной игрой. Новый живой тест `cargo test --bin sad-launcher live_effects_in_game -- --ignored --nocapture` (≈1 мин) сам запускает Minecraft 26.3 сразу в мир `SadTest` (`--quickPlaySingleplayer` через dev-переменную `MADRID_DEV_WORLD`, только для тестов), уводит фокус на панель задач (как будто игрок ушёл в лаунчер), переключает эффекты, возвращается в мир и проверяет по логу игры перезагрузку ресурсов и отсутствие ошибок шейдеров; затем автокликер в мире и в инвентаре с НАСТОЯЩИМ курсором на крестике окна. Мир `SadTest` (выживание → творческий, мирная, команды вкл.) создан один раз через интерфейс игры, лежит в `game/saves`. Помощники: `live_game_hold` (держит игру открытой), `live_post_input` (`MADRID_POST=lmb|rmb|f3|bright|plain`). Итог: всё зелёное, вживую видно — ночью яркость включается/выключается без перезапуска, автокликер прорубил тоннель.

Что нашли и починили (в 0.7.8 всё это было сломано, хотя юнит-тесты были зелёные):
1. **Страховка `incompatibleResourcePacks` выключала пак.** У Minecraft в `Options.loadSelectedResourcePacks` пак, который есть в этом списке и снова совместим, убирается из списка И пропускается в этот запуск (`else`-ветка не добавляет его в выбранные). Лаунчер добавлял его туда перед каждым запуском → пак стоял в «Доступных». Теперь лаунчер только УБИРАЕТ `file/SadEffects` из этого списка. Строка перезагрузки в 26.3: `Reloading ResourceManager: vanilla, vanilla, file/SadEffects` (два «vanilla» — норма для 26.x).
2. **Обёртка шейдера дописывалась в конец файла** → `fog.glsl:25: 'linear_fog_value' : no matching overloaded function` (в 26.x её зовёт `total_fog_value` в том же файле, GLSL требует объявления до вызова; плюс конец файла — за `#endif` guard). `wrap_function` теперь вставляет обёртку сразу после тела исходной функции.
3. **26.x — это SDL, а не GLFW** (`Created window using SDL video driver`, класс окна `SDL_app` — добавлен в поиск окна). SDL отдаёт клавиатуру только окну в фокусе: F3+T в фоне до игры не доходит вообще, Esc-трюк из 0.7.8 бесполезен. Теперь F3+T ставится в очередь (`AutomationHandle::request_reload`) и уходит в момент, когда игрок вернулся в мир: игра в фокусе и держит курсор (`cursor_captured`: курсор скрыт и `GetClipCursor` внутри окна игры — в мире 26.3 это прямоугольник 1×1 в центре, в инвентаре — весь экран и курсор виден). Меню паузы при этом закрыто — без Esc и без нажатий в меню. Статус: «Пак обновлён · включится, как только вернёшься в игру» → «Применено в игре · ресурсы перезагружены».
4. **Присланные клики (PostMessage) в мире игнорируются**: SDL в режиме захваченной мыши берёт кнопки из raw input (проверено: 10 присланных — блок цел, 1 настоящий — сломан). В инвентаре присланные клики работают (вкладка творческого инвентаря переключилась). Автокликер теперь: курсор захвачен → настоящий клик `SendInput` (Windows держит курсор внутри окна игры — на крестик или в другое окно попасть нельзя); курсор свободен (инвентарь/меню) → клик сообщением прямо в окно игры. Не в фокусе — не кликает (клики в фоне из 0.7.8 убраны: в 26.x до игры не доходят, а при меню паузы в центре экрана кнопка «Сохранить и выйти»).
- Ещё в логе 26.3: `Error parsing option value 15.0 for option Brightness … outside of range [0.0:1.0]` — гамма из options.txt в 26.x ограничена 1.0, поэтому яркость там работает только через шейдер (он и проверен вживую).
- ГРАБЛИ: Python на Windows в текстовом режиме пишет CRLF — коммит 0.7.8 перевёл 5 файлов в CRLF, а `"\0"` в heredoc стал настоящим NUL в effects.rs. Править файлы скриптами только через `open(..., 'rb'/'wb')`, проверять `grep -c $'\r'` и NUL.

## Обновление 2026-11-27 — v0.7.10: фикс вылета dontcry, отзыв пользователя (6 правок) и аудит багов

- **Фикс вылета модпака `dontcry` (`effects.rs`):** по логам `game-20261125-*.log` и `crash-2026-11-25_17.46.39-client.txt` выяснено, что с включённым «Без дождя/снега» (`no_weather`) `SadEffects` создавал `assets/minecraft/textures/environment/{rain,snow}.png` размером `16×16` вместо ванильных `64×256`. Мод Particle Rain (`particlerain-4.0.2+1.20.1-forge.jar`) при загрузке текстур делил высоту на 64 и падал с `IndexOutOfBoundsException: Cannot get rain_4 as sheet only contains 4 sprites`, срывая первый resource reload посреди загрузки Forge; аварийный повторный reload пере-вызывал `FMLCommonSetupEvent` с дублями каналов/фабрик и ронял клиент. Добавлена `transparent_png_wh(width, height)` — `rain.png`/`snow.png` генерируются в размере `64×256` (`TextureUtil.boostAlpha` в моде сразу выходит при нулевой альфе). Также исправлен `mc_minor_version` для календарных версий `26.x`.
- **6 правок по отзыву пользователя (скрин Telegram):**
  1. **Убрана единая папка сборок (`profiles.rs`, `config.rs`, `views/build.rs`, `views/play.rs`, `views/versions.rs`):** для каждой сборки и каждой запускаемой из каталога версий создаётся изолированная папка (`profiles/<id>/`, `upsert_version_profile`), добавлен выбор/смена произвольной папки (`custom_dir`, «Выбрать…» / «Сменить папку…» / «Стандартная»), кнопка «Папка игры» заменена на «Папка сборки» (`instance_dir()`).
  2. **Убрано ощущение скачивания «2 версий» (`minecraft.rs`, `views/versions.rs`):** в `InstalledVersion` добавлен флаг `base_only` (скрывает вспомогательную ванильную папку без `madrid.json`, если рядом установлен модлоадер для этой же версии MC), а этап загрузки базы при установке Forge/Fabric помечается как «База <mc>» / «Базовый клиент <mc>».
  3. **Улучшена подгрузка и рендер чанков (`launch.rs`):** в клиентские флаги G1GC добавлены `-XX:+UnlockExperimentalVMOptions`, `G1NewSizePercent=20`, `G1MaxNewSizePercent=40`, `G1HeapRegionSize=32M`, `G1ReservePercent=15`, `MaxGCPauseMillis=25` для ровного фреймтайма, а в `fps_defaults` выставлен `prioritizeChunkUpdates:1`.
  4. **Пофикшено сворачивание лаунчера и уточнён Pomme (`app.rs`, `views/versions.rs`):** удалено преждевременное `self.was_running = true` в `launch_to`, из-за которого лаунчер никогда не ловил переход `false → true` и оставался открытым позади игры; заголовок Pomme уточнён как «Pomme · внешний клиент на Rust + Vulkan».
  5. **Replay Mod / Flashback в 1 клик (`effects.rs`, `views/effects.rs`):** добавлена секция «ЗАПИСЬ РЕПЛЕЕВ (REPLAY MOD / FLASHBACK)» с установкой/удалением совместимого мода (`REPLAY_MOD`) и кнопкой открытия папки `replay_recordings` / `flashback` (`replay_dir`).
  6. **Плавные анимации без дёрганья и мыла (`app.rs`, `views/mods.rs`, `views/viewer.rs`, `views/effects.rs`):** убран `add_space(rise)` при переключении вкладок, убран сдвиг `max_rect` и оверфлоу-скейлинг карточек в `views/mods.rs`, исправлен `fade` иконок и `video_request` при закрытии просмотрщика, добавлен `TSTransform::IDENTITY` после завершения анимации `draw_remove_card` / `draw_about` + `request_repaint()` во время появления, включён мгновенный `auto_apply` для Custom Fog.
- **Аудит безопасности и стабильности:** защищён `uninstall_everything` (`is_dedicated_install_dir` + env-переменные без cmd-инъекций), `nickname_valid` считает символы по `.chars().count()`, `percent_decode` работает по байтам без UTF-8 паник, `mods::sanitize` блокирует `..` и ведущие точки (`ftb.rs` и `heal_broken_mod` используют его), `curse_detail` и `open_site` проверяют `http(s)://`, `launch::stop` ждёт завершения процесса (`kill`+`wait`) и сбрасывает PID, `launch` корректно определяет MC версию при нестандартном `id` и чинит сломанные моды внутри `launched_mods_dir`, `server.rs` проверяет совместимость лоадера/версии для `minecraft_server.jar` и `resolve_java`, `net.rs` обновляет `done` только после `write_at`, `packinfo.rs` валидирует размеры/границы ZIP, `upnp.rs` корректно парсит self-closing XML-теги и пропускает неполные `<service>`, `clear_for_reinstall` синхронно удаляет старые файлы при блокировке rename, в `settings.rs` исправлены ввод порта (`server_port_buf`), срабатывание слайдера памяти (`server_mem_saved`), фокус неактивных кнопок (`widgets.rs`), фоллбек шрифта (`theme.rs`) и переполнение времени (`room.rs`, `automation.rs`, `app.rs`), плюс тест `win64_input_struct_matches_winuser_abi` в `automation.rs` закрыт `#[cfg(all(windows, target_pointer_width = "64"))]` для сборки тестов на Linux/macOS CI.
