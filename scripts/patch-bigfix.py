import io

# ================= mods.rs: honest facets per kind =================
p = "src/mods.rs"
src = io.open(p, encoding="utf-8").read()
old = '''    let project_type = match kind {
        KIND_SHADER => "shader",
        KIND_RESOURCEPACK => "resourcepack",
        KIND_MODPACK => "modpack",
        _ => "mod",
    };
    let facets = if kind == KIND_MODPACK {
        format!(r#"[["project_type:modpack"],["versions:{}"]]"#, mc)
    } else {
        format!(
            r#"[["project_type:{}"],["versions:{}"],["categories:{}"]]"#,
            project_type, mc, loader
        )
    };'''
new = '''    let project_type = match kind {
        KIND_SHADER => "shader",
        KIND_RESOURCEPACK => "resourcepack",
        KIND_MODPACK => "modpack",
        _ => "mod",
    };
    // Only mods have loader categories on Modrinth; shaders/resourcepacks
    // use different category names, so filtering by them empties the list.
    let facets = match kind {
        KIND_MOD => format!(
            r#"[["project_type:mod"],["versions:{}"],["categories:{}"]]"#,
            mc, loader
        ),
        _ => format!(
            r#"[["project_type:{}"],["versions:{}"]]"#,
            project_type, mc
        ),
    };'''
assert old in src, "facets block not found"
src = src.replace(old, new, 1)

# pagination: offset + limit 40
src = src.replace('''pub async fn search(
    client: &reqwest::Client,
    cfg: &Config,
    query: &str,
    source: &str,
    mc: &str,
    loader: &str,
    kind: &str,
    sort: &str,
) -> Result<Vec<ModHit>> {''', '''pub async fn search(
    client: &reqwest::Client,
    cfg: &Config,
    query: &str,
    source: &str,
    mc: &str,
    loader: &str,
    kind: &str,
    sort: &str,
    offset: usize,
) -> Result<Vec<ModHit>> {''')

src = src.replace('''    let url = format!(
        "https://api.modrinth.com/v2/search?limit=20&index={}&query={}&facets={}",
        sort_q,
        enc(query),
        enc(&facets)
    );''', '''    let url = format!(
        "https://api.modrinth.com/v2/search?limit=40&offset={}&index={}&query={}&facets={}",
        offset,
        sort_q,
        enc(query),
        enc(&facets)
    );''')

# CF search: offset via index param
src = src.replace('''    let mut url = format!(
        "https://api.curseforge.com/v1/mods/search?gameId=432&classId={}&sortField={}&sortOrder=desc&pageSize=20&gameVersion={}&searchFilter={}",
        class_id,
        sort_field,
        enc(mc),
        enc(query)
    );''', '''    let page = offset / 20;
    let mut url = format!(
        "https://api.curseforge.com/v1/mods/search?gameId=432&classId={}&sortField={}&sortOrder=desc&pageSize=20&index={}&gameVersion={}&searchFilter={}",
        class_id,
        sort_field,
        page,
        enc(mc),
        enc(query)
    );''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("mods facets+paging ok")

# ================= app.rs: SearchState offset, load-more, loader switch refresh
p = "src/app.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''    /// 0 = downloads, 1 = follows (популярность), 2 = updated, 3 = newest.
    pub sort: usize,''', '''    /// 0 = downloads, 1 = follows (популярность), 2 = updated, 3 = newest.
    pub sort: usize,
    /// Pagination: next Modrinth/CF page offset.
    pub offset: usize,''')
src = src.replace('''            query: String::new(),
            source: 1,
            kind: 0,
            sort: 0,''', '''            query: String::new(),
            source: 1,
            kind: 0,
            sort: 0,
            offset: 0,''')

src = src.replace('''        let sort = match self.search.sort {
            1 => "follows",
            2 => "updated",
            3 => "newest",
            _ => "downloads",
        }
        .to_string();''', '''        let sort = match self.search.sort {
            1 => "follows",
            2 => "updated",
            3 => "newest",
            _ => "downloads",
        }
        .to_string();
        let offset = self.search.offset;''')

src = src.replace('''                    let result = mods::search(&client, &cfg, &query, &source, &mc, &loader, &kind, &sort)
                        .await
                        .map_err(|e| format!("{e:#}"));
                    let _ = tx.send(result);
                    ctx.request_repaint();''', '''                    let result = mods::search(&client, &cfg, &query, &source, &mc, &loader, &kind, &sort, offset)
                        .await
                        .map_err(|e| format!("{e:#}"));
                    let _ = tx.send(result);
                    ctx.request_repaint();''')

# search results: append when loading more, reset catalog_at
src = src.replace('''            match result {
                Ok(list) => {
                    self.search.results = list;
                    self.search.error = None;
                }
                Err(e) => {
                    self.search.results.clear();
                    self.search.error = Some(e);
                }
            }''', '''            match result {
                Ok(list) => {
                    let fresh = list.len();
                    if offset > 0 {
                        let known = self
                            .search
                            .results
                            .iter()
                            .map(|h| (h.source.clone(), h.slug.clone()))
                            .collect::<std::collections::HashSet<_>>();
                        for h in list {
                            if !known.contains(&(h.source.clone(), h.slug.clone())) {
                                self.search.results.push(h);
                            }
                        }
                    } else {
                        self.search.results = list;
                    }
                    self.search.offset += fresh.max(1);
                    self.search.error = None;
                    self.catalog_at = Instant::now();
                }
                Err(e) => {
                    if self.search.offset == 0 {
                        self.search.results.clear();
                    }
                    self.search.error = Some(e);
                }
            }''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("app paging ok")

# ================= rustmc.rs: patch the vsync-close bug after clone =================
p = "src/rustmc.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''    let repo_dir = repo();
    let git = repo_dir.join(".git").is_dir();
    if git {''', '''    let repo_dir = repo();
    patch_stevenarella(&repo_dir);
    let git = repo_dir.join(".git").is_dir();
    if git {''')

src += '''
/// Stevenarella closes on a stale vsync check right after boot; keep it open.
fn patch_stevenarella(repo: &PathBuf) {
    let main_rs = repo.join("src").join("main.rs");
    let Ok(text) = std::fs::read_to_string(&main_rs) else {
        return;
    };
    let broken = "error!(\\"Changing vsync currently requires restarting\\");\\n        game.should_close = true;";
    let fixed = "error!(\\"Changing vsync currently requires restarting\\");\\n        *vsync = vsync_changed;";
    if text.contains(broken) {
        let patched = text.replace(&broken, &fixed);
        let _ = std::fs::write(&main_rs, patched);
    }
}
'''
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("rustmc patch ok")
