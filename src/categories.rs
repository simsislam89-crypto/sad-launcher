//! Catalog categories per source and content kind, with Russian names.
//! Modrinth uses slugs (`adventure`), CurseForge numeric ids (`4475`).
//! The tables are static: the filter works offline and instantly.

use crate::mods::{KIND_MAP, KIND_MOD, KIND_MODPACK, KIND_RESOURCEPACK, KIND_SHADER};

type Table = &'static [(&'static str, &'static str)];

const MR_MODPACK: Table = &[
    ("adventure", "Приключения"),
    ("magic", "Магия"),
    ("technology", "Техника"),
    ("quests", "Квесты"),
    ("combat", "Сражения"),
    ("challenging", "Хардкор"),
    ("kitchen-sink", "Всё и сразу"),
    ("lightweight", "Лёгкие"),
    ("multiplayer", "Мультиплеер"),
    ("optimization", "Оптимизация"),
];

const MR_MOD: Table = &[
    ("adventure", "Приключения"),
    ("magic", "Магия"),
    ("technology", "Техника"),
    ("worldgen", "Генерация мира"),
    ("mobs", "Мобы"),
    ("equipment", "Снаряжение"),
    ("decoration", "Декор"),
    ("food", "Еда"),
    ("storage", "Хранение"),
    ("transportation", "Транспорт"),
    ("game-mechanics", "Механики"),
    ("optimization", "Оптимизация"),
    ("utility", "Утилиты"),
    ("library", "Библиотеки"),
    ("management", "Управление"),
    ("economy", "Экономика"),
    ("social", "Общение"),
    ("minigame", "Мини-игры"),
    ("cursed", "Странное"),
];

const MR_SHADER: Table = &[
    ("realistic", "Реалистичные"),
    ("semi-realistic", "Полуреализм"),
    ("vanilla-like", "Как ванила"),
    ("fantasy", "Фэнтези"),
    ("cartoon", "Мультяшные"),
    ("potato", "Для слабых ПК"),
    ("low", "Лёгкие"),
    ("medium", "Средние"),
    ("high", "Тяжёлые"),
    ("shadows", "Тени"),
    ("reflections", "Отражения"),
    ("path-tracing", "Трассировка лучей"),
    ("colored-lighting", "Цветной свет"),
    ("pbr", "PBR"),
    ("atmosphere", "Атмосфера"),
    ("foliage", "Листва"),
    ("bloom", "Свечение"),
    ("cursed", "Странное"),
];

const MR_RESOURCEPACK: Table = &[
    ("vanilla-like", "Как ванила"),
    ("realistic", "Реалистичные"),
    ("simplistic", "Простые"),
    ("themed", "Тематические"),
    ("tweaks", "Твики"),
    ("utility", "Утилиты"),
    ("decoration", "Декор"),
    ("combat", "PvP"),
    ("modded", "Для модов"),
    ("audio", "Звуки"),
    ("gui", "Интерфейс"),
    ("fonts", "Шрифты"),
    ("models", "Модели"),
    ("16x", "16x"),
    ("32x", "32x"),
    ("64x", "64x"),
    ("128x", "128x"),
    ("cursed", "Странное"),
];

const CF_MODPACK: Table = &[
    ("4475", "RPG и приключения"),
    ("4473", "Магия"),
    ("4472", "Техника"),
    ("4478", "Квесты"),
    ("4476", "Исследование"),
    ("4483", "PvP"),
    ("4479", "Хардкор"),
    ("9243", "Эксперт"),
    ("4736", "Скайблок"),
    ("7418", "Хоррор"),
    ("4474", "Sci-Fi"),
    ("4484", "Мультиплеер"),
    ("4477", "Мини-игры"),
    ("4480", "На карте"),
    ("4481", "Лёгкие"),
    ("4482", "Огромные"),
    ("5128", "Vanilla+"),
    ("4487", "FTB"),
];

const CF_MOD: Table = &[
    ("422", "RPG и приключения"),
    ("419", "Магия"),
    ("412", "Техника"),
    ("406", "Генерация мира"),
    ("407", "Биомы"),
    ("409", "Структуры"),
    ("410", "Измерения"),
    ("411", "Мобы"),
    ("434", "Броня и оружие"),
    ("436", "Еда"),
    ("416", "Фермерство"),
    ("420", "Хранение"),
    ("408", "Руды и ресурсы"),
    ("4558", "Редстоун"),
    ("423", "Карта и инфо"),
    ("424", "Косметика"),
    ("6814", "Производительность"),
    ("6821", "Исправления"),
    ("5191", "Утилиты"),
    ("421", "Библиотеки"),
    ("426", "Аддоны"),
    ("6484", "Create"),
    ("10775", "Хоррор"),
    ("435", "Для сервера"),
    ("425", "Разное"),
];

const CF_RESOURCEPACK: Table = &[
    ("393", "16x"),
    ("394", "32x"),
    ("395", "64x"),
    ("396", "128x"),
    ("397", "256x"),
    ("398", "512x+"),
    ("400", "Фотореализм"),
    ("403", "Традиционные"),
    ("401", "Современные"),
    ("402", "Средневековье"),
    ("399", "Стимпанк"),
    ("404", "Анимированные"),
    ("4465", "Для модов"),
    ("5244", "Шрифты"),
    ("5193", "Датапаки"),
    ("405", "Разное"),
];

const CF_SHADER: Table = &[("6553", "Реалистичные"), ("6555", "Как ванила"), ("6554", "Фэнтези")];

const CF_MAP: Table = &[
    ("248", "Приключения"),
    ("253", "Выживание"),
    ("249", "Творчество"),
    ("251", "Паркур"),
    ("252", "Головоломки"),
    ("250", "Мини-игры"),
    ("4464", "Модовые миры"),
];

/// Categories offered for a source + kind, in display order.
pub fn list(source: &str, kind: &str) -> Table {
    match (source, kind) {
        ("curseforge", KIND_MODPACK) => CF_MODPACK,
        ("curseforge", KIND_MOD) => CF_MOD,
        ("curseforge", KIND_RESOURCEPACK) => CF_RESOURCEPACK,
        ("curseforge", KIND_SHADER) => CF_SHADER,
        (_, KIND_MAP) => CF_MAP,
        (_, KIND_MODPACK) => MR_MODPACK,
        (_, KIND_MOD) => MR_MOD,
        (_, KIND_RESOURCEPACK) => MR_RESOURCEPACK,
        (_, KIND_SHADER) => MR_SHADER,
        _ => &[],
    }
}

/// Russian name of one category id, if known.
pub fn label(source: &str, kind: &str, id: &str) -> Option<&'static str> {
    list(source, kind).iter().find(|(k, _)| *k == id).map(|(_, v)| *v)
}

/// Known categories of a project as Russian names (max three, no dupes).
/// Loader tags (`forge`, `fabric`) and unknown ids are skipped.
pub fn labels(source: &str, kind: &str, ids: impl Iterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in ids {
        if let Some(name) = label(source, kind, &id) {
            if !out.iter().any(|o| o == name) {
                out.push(name.to_string());
            }
        }
        if out.len() >= 3 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_skip_loaders_and_unknown() {
        let ids = ["forge", "adventure", "magic", "fabric", "zzz"].map(String::from);
        assert_eq!(labels("modrinth", KIND_MODPACK, ids.into_iter()), vec!["Приключения", "Магия"]);
        let cf = ["4475", "4473"].map(String::from);
        assert_eq!(labels("curseforge", KIND_MODPACK, cf.into_iter()), vec!["RPG и приключения", "Магия"]);
    }

    #[test]
    fn maps_always_curseforge() {
        assert_eq!(list("modrinth", KIND_MAP)[0].0, "248");
    }
}
