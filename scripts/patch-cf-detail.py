import io

p = "src/mods.rs"
src = io.open(p, encoding="utf-8").read()

# 1) CF: Forge loader filter for modpacks too (mods had it already)
src = src.replace('''    if kind == KIND_MOD {
        url.push_str(&format!("&modLoaderType=1")); // Forge
    }
    let _ = loader;''', '''    if kind == KIND_MOD || kind == KIND_MODPACK {
        url.push_str(&format!("&modLoaderType=1")); // Forge — the Madrid loader
    }
    let _ = loader;''')

# 2) ProjectDetail gains versions + compatibility
src = src.replace('''#[derive(Clone, Serialize)]
pub struct ProjectDetail {
    pub title: String,
    pub summary: String,
    pub body: String,
    pub downloads: u64,
    pub author: String,
    pub page_url: String,
    pub icon_url: Option<String>,
}''', '''#[derive(Clone, Serialize)]
pub struct ProjectVersion {
    pub name: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
}

#[derive(Clone, Serialize)]
pub struct ProjectDetail {
    pub title: String,
    pub summary: String,
    pub body: String,
    pub downloads: u64,
    pub author: String,
    pub page_url: String,
    pub icon_url: Option<String>,
    /// Latest versions for the «версии» section.
    pub versions: Vec<ProjectVersion>,
    /// True when any version targets the Madrid profile (1.20.1 / forge).
    pub compatible: Option<bool>,
}''')

# 3) Modrinth detail: fetch versions, evaluate compatibility
src = src.replace('''        Ok(ProjectDetail {
            title: proj.title,
            summary: proj.description,
            body: clean_md(&proj.body),
            downloads: proj.downloads,
            author: hit.author.clone(),
            page_url: format!("https://modrinth.com/mod/{}", hit.slug),
            icon_url: proj.icon_url,
        })''', '''        let (versions, compatible) = match project_versions(client, &hit.slug).await {
            Ok(list) => {
                let compatible = list.iter().any(|v| {
                    v.game_versions.iter().any(|g| g == crate::config::MADRID_MC)
                        && v.loaders.iter().any(|l| l == "forge")
                });
                let mut versions = list;
                versions.truncate(8);
                (versions, Some(compatible))
            }
            Err(_) => (Vec::new(), None),
        };
        Ok(ProjectDetail {
            title: proj.title,
            summary: proj.description,
            body: clean_md(&proj.body),
            downloads: proj.downloads,
            author: hit.author.clone(),
            page_url: format!("https://modrinth.com/mod/{}", hit.slug),
            icon_url: proj.icon_url,
            versions,
            compatible,
        })''')

# 4) CurseForge detail: real /v1/mods + /description (key required)
src = src.replace('''    } else {
        bail!("Карточка CurseForge открывается на сайте — ключ нужен только для поиска");
    }
}''', '''    } else {
        if cfg_cf_key_missing() {
            bail!("Для карточки CurseForge нужен ключ API в настройках (поиск и карточки). Скачивание работает и без него.");
        }
        #[derive(Deserialize)]
        struct Mod {
            name: String,
            #[serde(default)]
            summary: String,
            #[serde(default, rename = "downloadCount")]
            download_count: u64,
            #[serde(default)]
            logo: Option<Logo>,
            #[serde(default)]
            authors: Vec<Author>,
        }
        #[derive(Deserialize)]
        struct Logo {
            #[serde(default)]
            url: String,
        }
        #[derive(Deserialize)]
        struct Author {
            name: String,
        }
        #[derive(Deserialize)]
        struct Resp {
            data: Mod,
        }
        let key = crate::cf_api_key_from_settings();
        let resp: Resp = client
            .get(format!("https://api.curseforge.com/v1/mods/{}", hit.id))
            .header("x-api-key", key.clone())
            .send()
            .await
            .context("CurseForge не отвечает")?
            .error_for_status()
            .context("CurseForge отклонил запрос (проверь ключ)")?
            .json()
            .await
            .context("Проект не читается")?;
        #[derive(Deserialize)]
        struct DescResp {
            data: String,
        }
        let description = match client
            .get(format!("https://api.curseforge.com/v1/mods/{}/description", hit.id))
            .header("x-api-key", key.clone())
            .send()
            .await
        {
            Ok(r) => r
                .json::<DescResp>()
                .await
                .map(|d| clean_html(&d.data))
                .unwrap_or_default(),
            Err(_) => String::new(),
        };
        let m = resp.data;
        Ok(ProjectDetail {
            title: m.name,
            summary: m.summary,
            body: description,
            downloads: m.download_count,
            author: m.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
            page_url: format!("https://www.curseforge.com/projects/{}", hit.id),
            icon_url: m.logo.map(|l| l.url).filter(|s| !s.is_empty()),
            versions: Vec::new(),
            compatible: None,
        })
    }
}

fn cfg_cf_key_missing() -> bool {
    crate::cf_api_key_from_settings().is_empty()
}

/// The CurseForge API key from the persisted settings file.
fn cf_api_key_from_settings() -> String {
    crate::config::load().cf_api_key
}

/// Strips HTML tags so the description reads as plain text.
fn clean_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Latest versions of a Modrinth project (for the versions section).
async fn project_versions(
    client: &reqwest::Client,
    slug: &str,
) -> Result<Vec<ProjectVersion>> {
    #[derive(Deserialize)]
    struct V {
        name: String,
        #[serde(default)]
        game_versions: Vec<String>,
        #[serde(default)]
        loaders: Vec<String>,
    }
    let list: Vec<V> = client
        .get(format!("https://api.modrinth.com/v2/project/{}/version", slug))
        .send()
        .await
        .context("Список версий не ответил")?
        .error_for_status()?
        .json()
        .await
        .context("Список версий не читается")?;
    Ok(list
        .into_iter()
        .map(|v| ProjectVersion {
            name: v.name,
            game_versions: v.game_versions,
            loaders: v.loaders,
        })
        .collect())
}''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("mods detail ok")
