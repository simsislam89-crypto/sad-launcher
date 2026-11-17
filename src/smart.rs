//! Smart catalog search: understands Russian and English, typos and the
//! wrong keyboard layout.
//!
//! «проминсе», «promince», «зкщьштуср» all find Prominence:
//! 1. the query is turned into Latin forms — a small RU→EN dictionary
//!    («мини карта» → «minimap»), phonetic transliteration («проминсе» →
//!    «prominse») and the ЙЦУКЕН→QWERTY layout swap («зкщьшт» → «promin»);
//! 2. a shortened prefix of the best form is searched too — the catalogs
//!    match prefixes, so «promin» reaches «prominence» despite the typo;
//! 3. merged results are re-ranked locally by fuzzy similarity
//!    (Jaro–Winkler on the title and its words), the catalog's own order
//!    breaking ties.

/// Whether the text contains Cyrillic letters.
pub fn has_cyrillic(text: &str) -> bool {
    text.chars().any(|c| matches!(c, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё'))
}

/// Lowercase, letters/digits/spaces only, single spaces.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = true;
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(if c == 'ё' { 'е' } else { c });
            space = false;
        } else if !space {
            out.push(' ');
            space = true;
        }
    }
    out.trim_end().to_string()
}

/// Phonetic Russian → Latin, tuned for mod names written by ear.
pub fn translit(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars().flat_map(char::to_lowercase) {
        let s: &str = match c {
            'а' => "a",
            'б' => "b",
            'в' => "v",
            'г' => "g",
            'д' => "d",
            'е' | 'ё' | 'э' => "e",
            'ж' => "zh",
            'з' => "z",
            'и' | 'й' | 'ы' => "i",
            'к' => "k",
            'л' => "l",
            'м' => "m",
            'н' => "n",
            'о' => "o",
            'п' => "p",
            'р' => "r",
            'с' => "s",
            'т' => "t",
            'у' => "u",
            'ф' => "f",
            'х' => "h",
            'ц' => "c",
            'ч' => "ch",
            'ш' => "sh",
            'щ' => "sch",
            'ъ' | 'ь' => "",
            'ю' => "yu",
            'я' => "ya",
            other => {
                out.push(other);
                continue;
            }
        };
        out.push_str(s);
    }
    out
}

/// Text typed with the Russian layout active while meaning English keys.
pub fn layout_swap(text: &str) -> String {
    const RU: &str = "йцукенгшщзхъфывапролджэячсмитьбю";
    const EN: &str = "qwertyuiop[]asdfghjkl;'zxcvbnm,.";
    let map: Vec<(char, char)> = RU.chars().zip(EN.chars()).collect();
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| map.iter().find(|(r, _)| *r == c).map(|(_, e)| *e).unwrap_or(c))
        .collect()
}

/// Common Russian search words → what mod pages call them.
const DICT: &[(&str, &str)] = &[
    ("мини карта", "minimap"),
    ("миникарта", "minimap"),
    ("карта мира", "world map"),
    ("шейдеры", "shaders"),
    ("шейдер", "shader"),
    ("текстуры", "textures"),
    ("ресурспак", "resource pack"),
    ("оптимизация", "optimization"),
    ("фпс", "fps"),
    ("производительность", "performance"),
    ("магия", "magic"),
    ("техника", "tech"),
    ("технологии", "technology"),
    ("механизмы", "create"),
    ("дракон", "dragon"),
    ("драконы", "dragons"),
    ("оружие", "weapons"),
    ("пушки", "guns"),
    ("мечи", "swords"),
    ("броня", "armor"),
    ("мебель", "furniture"),
    ("декор", "decoration"),
    ("машины", "vehicles"),
    ("транспорт", "vehicles"),
    ("самолеты", "planes"),
    ("еда", "food"),
    ("ферма", "farming"),
    ("мобы", "mobs"),
    ("животные", "animals"),
    ("питомцы", "pets"),
    ("звуки", "sounds"),
    ("музыка", "music"),
    ("реализм", "realistic"),
    ("выживание", "survival"),
    ("хоррор", "horror"),
    ("ужасы", "horror"),
    ("зомби", "zombie"),
    ("боссы", "bosses"),
    ("подземелья", "dungeons"),
    ("данжи", "dungeons"),
    ("замки", "castles"),
    ("структуры", "structures"),
    ("биомы", "biomes"),
    ("деревья", "trees"),
    ("руды", "ores"),
    ("космос", "space"),
    ("квесты", "quests"),
    ("приключения", "adventure"),
    ("рпг", "rpg"),
    ("строительство", "building"),
    ("инвентарь", "inventory"),
    ("сундуки", "storage"),
    ("хранилище", "storage"),
    ("рецепты", "recipes"),
    ("джей", "jei"),
    ("скайблок", "skyblock"),
    ("пвп", "pvp"),
    ("голод", "hunger"),
    ("освещение", "lighting"),
    ("тени", "shadows"),
    ("вода", "water"),
    ("погода", "weather"),
    ("сезоны", "seasons"),
    ("библиотека", "library"),
];

fn dictionary(norm: &str) -> Option<String> {
    let mut out = norm.to_string();
    let mut hit = false;
    // Longest phrases first so «мини карта» wins over «карта».
    let mut entries: Vec<&(&str, &str)> = DICT.iter().collect();
    entries.sort_by_key(|(ru, _)| std::cmp::Reverse(ru.len()));
    for (ru, en) in entries {
        if out.contains(ru) {
            out = out.replace(ru, en);
            hit = true;
        }
    }
    (hit && !has_cyrillic(&out)).then_some(out)
}

/// Looks like a word, not keyboard noise (layout-swap check).
fn plausible_latin(text: &str) -> bool {
    let letters: Vec<char> = text.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    if letters.len() < 3 || text.chars().any(|c| matches!(c, '[' | ']' | ';' | '\'' | ',' | '.')) {
        return false;
    }
    let vowels = letters.iter().filter(|c| "aeiouy".contains(**c)).count();
    vowels * 5 >= letters.len()
}

/// Latin forms of the query, best first (no duplicates). The first entry
/// is what the catalog is asked for primarily.
pub fn latin_forms(query: &str) -> Vec<String> {
    let norm = normalize(query);
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: String| {
        let s = s.trim().to_string();
        if !s.is_empty() && !out.contains(&s) {
            out.push(s);
        }
    };
    if !has_cyrillic(&norm) {
        push(norm);
        return out;
    }
    if let Some(d) = dictionary(&norm) {
        push(d);
    }
    let swapped = layout_swap(&norm);
    if plausible_latin(&swapped) {
        push(swapped);
    }
    push(translit(&norm));
    out
}

/// A shorter prefix that survives typos at the end of a long single word
/// («prominse» → «promin»). None for short or multi-word queries.
pub fn typo_prefix(form: &str) -> Option<String> {
    if form.contains(' ') {
        return None;
    }
    let n = form.chars().count();
    if n < 6 {
        return None;
    }
    let keep = (n * 7).div_ceil(10).max(4);
    Some(form.chars().take(keep).collect())
}

/// Requests to send for a query: (text, is_primary). At most three.
pub fn query_plan(query: &str) -> Vec<String> {
    let forms = latin_forms(query);
    let mut plan: Vec<String> = Vec::new();
    let original = query.trim().to_string();
    let mut add = |s: String| {
        if !s.is_empty() && !plan.contains(&s) && plan.len() < 3 {
            plan.push(s);
        }
    };
    if let Some(first) = forms.first() {
        add(first.clone());
    }
    if let Some(p) = forms.first().and_then(|f| typo_prefix(f)) {
        add(p);
    }
    for f in forms.iter().skip(1) {
        add(f.clone());
    }
    // Russian titles exist too (resource packs, maps): keep the original.
    if has_cyrillic(&original) {
        add(original);
    }
    plan
}

/// Jaro–Winkler similarity, 0..1.
pub fn jaro_winkler(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let window = (a.len().max(b.len()) / 2).saturating_sub(1);
    let mut a_match = vec![false; a.len()];
    let mut b_match = vec![false; b.len()];
    let mut matches = 0usize;
    for i in 0..a.len() {
        let lo = i.saturating_sub(window);
        let hi = (i + window + 1).min(b.len());
        for j in lo..hi {
            if !b_match[j] && a[i] == b[j] {
                a_match[i] = true;
                b_match[j] = true;
                matches += 1;
                break;
            }
        }
    }
    if matches == 0 {
        return 0.0;
    }
    let mut transpositions = 0usize;
    let mut k = 0usize;
    for i in 0..a.len() {
        if a_match[i] {
            while !b_match[k] {
                k += 1;
            }
            if a[i] != b[k] {
                transpositions += 1;
            }
            k += 1;
        }
    }
    let m = matches as f64;
    let jaro = (m / a.len() as f64 + m / b.len() as f64 + (m - transpositions as f64 / 2.0) / m) / 3.0;
    let prefix = a.iter().zip(b.iter()).take(4).take_while(|(x, y)| x == y).count() as f64;
    jaro + prefix * 0.1 * (1.0 - jaro)
}

/// How well a title answers the query forms, 0..1.
pub fn score(title: &str, slug: &str, forms: &[String]) -> f64 {
    let title = normalize(title);
    let slug = normalize(&slug.replace(['-', '_'], " "));
    let mut best = 0.0f64;
    for form in forms {
        if form.is_empty() {
            continue;
        }
        for hay in [&title, &slug] {
            if hay.is_empty() {
                continue;
            }
            if hay == form {
                return 1.0;
            }
            if hay.starts_with(form.as_str()) {
                best = best.max(0.97);
                continue;
            }
            if hay.contains(form.as_str()) {
                best = best.max(0.92);
                continue;
            }
            best = best.max(jaro_winkler(form, hay) * 0.95);
            // Single-word query against each title word, and against the
            // word's prefix of the same length («promince» vs «prominence»).
            for word in hay.split(' ') {
                let jw = jaro_winkler(form, word);
                let n = form.chars().count();
                let head: String = word.chars().take(n + 1).collect();
                best = best.max(jw.max(jaro_winkler(form, &head) * 0.97) * 0.95);
            }
        }
    }
    best
}

/// Re-ranks merged hits: clearly better matches rise, otherwise the
/// catalog's order (relevance / downloads) is kept.
pub fn rerank<T>(items: Vec<T>, query: &str, key: impl Fn(&T) -> (String, String)) -> Vec<T> {
    let mut forms = latin_forms(query);
    let original = normalize(query);
    if !forms.contains(&original) {
        forms.push(original);
    }
    let mut scored: Vec<(i64, usize, T)> = items
        .into_iter()
        .enumerate()
        .map(|(i, item)| {
            let (title, slug) = key(&item);
            // Buckets of 0.05 keep near-equal matches in catalog order.
            let bucket = (score(&title, &slug, &forms) * 20.0).round() as i64;
            (bucket, i, item)
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, t)| t).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translit_by_ear() {
        assert_eq!(translit("проминсе"), "prominse");
        assert_eq!(translit("крейт"), "kreit");
    }

    #[test]
    fn wrong_layout_is_fixed() {
        assert_eq!(layout_swap("зкщьштутсу"), "prominence");
        assert_eq!(latin_forms("зкщьштутсу")[0], "prominence");
    }

    #[test]
    fn dictionary_first() {
        assert_eq!(latin_forms("мини карта")[0], "minimap");
        assert_eq!(latin_forms("шейдеры")[0], "shaders");
    }

    #[test]
    fn plan_has_prefix_for_typos() {
        let plan = query_plan("проминсе");
        assert_eq!(plan[0], "prominse");
        assert!(plan.contains(&"promin".to_string()), "{plan:?}");
        let plan = query_plan("promince");
        assert_eq!(plan, vec!["promince".to_string(), "promin".to_string()]);
        assert_eq!(query_plan("jei"), vec!["jei".to_string()]);
    }

    #[test]
    fn prominence_ranks_first() {
        let titles = vec![
            ("Prometheus Pack", "prometheus"),
            ("Better Minecraft", "better-mc"),
            ("Prominence II RPG: Hasturian Era", "prominence-2"),
        ];
        for q in ["проминсе", "promince", "prominence", "зкщьштутсу"] {
            let out = rerank(titles.clone(), q, |t| (t.0.to_string(), t.1.to_string()));
            assert_eq!(out[0].1, "prominence-2", "query {q}: {:?}", out);
        }
    }

    #[test]
    fn exact_beats_fuzzy_and_order_kept() {
        let titles = vec![("JEI Integration", "a"), ("Just Enough Items (JEI)", "jei"), ("Jade", "jade")];
        let out = rerank(titles, "jei", |t| (t.0.to_string(), t.1.to_string()));
        assert_eq!(out[0].1, "jei");
    }
}
