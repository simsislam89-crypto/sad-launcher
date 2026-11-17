import io

p = "src/mods.rs"
src = io.open(p, encoding="utf-8").read()

# 1) base host helper: official API with key, public mirror without
helper = '''
/// CurseForge host: the official API needs a key; the public mirror
/// (api.curse.tools) serves the same v1 schema without one.
fn cf_base() -> &'static str {
    if crate::config::load().cf_api_key.trim().is_empty() {
        "https://api.curse.tools/v1/cf"
    } else {
        "https://api.curseforge.com/v1"
    }
}

fn cf_auth_headers(cfg: &Config) -> Option<String> {
    let key = cfg.cf_api_key.trim().to_string();
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}
'''
anchor = "pub const KIND_MOD: &str = \"mod\";"
assert anchor in src
src = src.replace(anchor, helper.strip() + "\n\n" + anchor, 1)

# 2) replace hosts everywhere
src = src.replace('"https://api.curseforge.com/v1/mods/search', '"{}/mods/search')
src = src.replace('"https://api.curseforge.com/v1/mods/{}/description"', '"{}/mods/{}/description"')
src = src.replace('"https://api.curseforge.com/v1/mods/{}", hit.id', '"{}/mods/{}", hit.id')
src = src.replace('"https://api.curseforge.com/v1/mods/{}/files?gameVersion={}&pageSize=10"', '"{}/mods/{}/files?gameVersion={}&pageSize=10"')
src = src.replace('"https://api.curseforge.com/v1/mods/{}/files/{}/download-url"', '"{}/mods/{}/files/{}/download-url"')

# format!() call sites need cf_base() as first argument now
src = src.replace('''    let mut url = format!(
        "https://api.curseforge.com/v1/mods/search?gameId=432&classId={}&sortField={}&sortOrder=desc&pageSize=20&index={}&gameVersion={}&searchFilter={}",
        class_id,
        sort_field,
        page,
        enc(mc),
        enc(query)
    );''', '''    let mut url = format!(
        "{}/mods/search?gameId=432&classId={}&sortField={}&sortOrder=desc&pageSize=20&index={}&gameVersion={}&searchFilter={}",
        cf_base(),
        class_id,
        sort_field,
        page,
        enc(mc),
        enc(query)
    );''')

src = src.replace('''        let resp: Resp = client
            .get(format!("{}/mods/search", cf_base()))''', '''        let resp: Resp = client
            .get(format!("{}/mods/search", cf_base()))''')

# the two format! sites that still take hit.id / file.id in URL path
src = src.replace('''        client
            .get(format!("https://api.curseforge.com/v1/mods/{}", hit.id))''', '''        client
            .get(format!("{}/mods/{}", cf_base(), hit.id))''')

src = src.replace('''            .get(format!(
                "https://api.curseforge.com/v1/mods/{}/description", hit.id
            ))''', '''            .get(format!(
                "{}/mods/{}/description",
                cf_base(),
                hit.id
            ))''')

src = src.replace('''    let mut url = format!(
        "https://api.curseforge.com/v1/mods/{}/files?gameVersion={}&pageSize=10",
        hit.id,
        enc(mc)
    );''', '''    let mut url = format!(
        "{}/mods/{}/files?gameVersion={}&pageSize=10",
        cf_base(),
        hit.id,
        enc(mc)
    );''')

src = src.replace('''        let resolved: UrlResp = client
            .get(format!(
                "https://api.curseforge.com/v1/mods/{}/files/{}/download-url",
                file.project_id, file.file_id
            ))''', '''        let resolved: UrlResp = client
            .get(format!(
                "{}/mods/{}/files/{}/download-url",
                cf_base(),
                file.project_id,
                file.file_id
            ))''')

src = src.replace('''    let resolved: UrlResp = client
        .get(format!(
            "https://api.curseforge.com/v1/mods/{}/files/{}/download-url",
            hit.id, file.id
        ))''', '''    let resolved: UrlResp = client
        .get(format!(
            "{}/mods/{}/files/{}/download-url",
            cf_base(),
            hit.id,
            file.id
        ))''')

# 3) serde aliases: mirror is camelCase, official v1 snake_case
src = src.replace('''        #[serde(default, rename = "download_count")]
        download_count: u64,''', '''        #[serde(default, rename = "download_count", alias = "downloadCount")]
        download_count: u64,''')
src = src.replace('''        #[serde(default, rename = "latest_files")]
        latest_files: Vec<File>,''', '''        #[serde(default, rename = "latest_files", alias = "latestFiles")]
        latest_files: Vec<File>,''')
src = src.replace('''        #[serde(default, rename = "thumbnailUrl")]
        thumbnail_url: String,''', '''        #[serde(default, rename = "thumbnailUrl", alias = "thumbnail_url")]
        thumbnail_url: String,''')

# 4) no key required anymore for CF detail — remove the bail
src = src.replace('''        if cfg_cf_key_missing() {
            bail!("Для карточки CurseForge нужен ключ API в настройках (поиск и карточки). Скачивание работает и без него.");
        }
''', '')

# cf_api_key_from_settings helper stays; cfg_cf_key_missing no longer used
src = src.replace('''fn cfg_cf_key_missing() -> bool {
    crate::cf_api_key_from_settings().is_empty()
}

''', '')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("cf mirror ok")
