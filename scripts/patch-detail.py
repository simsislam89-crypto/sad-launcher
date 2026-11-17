import io

# ============ mods.rs: detail fetch for Modrinth + CF fallback page url
p = "src/mods.rs"
src = io.open(p, encoding="utf-8").read()

detail = '''
// ---------------------------------------------------------------- project card

#[derive(Clone, Serialize)]
pub struct ProjectDetail {
    pub title: String,
    pub summary: String,
    pub body: String,
    pub downloads: u64,
    pub author: String,
    pub page_url: String,
    pub icon_url: Option<String>,
}

/// Loads the full project card: description, author, downloads, site link.
pub async fn detail(client: &reqwest::Client, hit: &ModHit) -> Result<ProjectDetail> {
    if hit.source == "modrinth" {
        #[derive(Deserialize)]
        struct Proj {
            title: String,
            #[serde(default)]
            description: String,
            #[serde(default)]
            body: String,
            #[serde(default)]
            downloads: u64,
            #[serde(default, rename = "icon_url")]
            icon_url: Option<String>,
        }
        let proj: Proj = client
            .get(format!("https://api.modrinth.com/v2/project/{}", enc(&hit.slug)))
            .send()
            .await
            .context("Modrinth не отвечает")?
            .error_for_status()?
            .json()
            .await
            .context("Проект не читается")?;
        Ok(ProjectDetail {
            title: proj.title,
            summary: proj.description,
            body: clean_md(&proj.body),
            downloads: proj.downloads,
            author: hit.author.clone(),
            page_url: format!("https://modrinth.com/mod/{}", hit.slug),
            icon_url: proj.icon_url,
        })
    } else {
        bail!("Карточка CurseForge открывается на сайте — ключ нужен только для поиска");
    }
}

/// Strips markdown noise so the body reads as plain text.
fn clean_md(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut in_code = false;
    for line in md.lines() {
        let mut line = line.to_string();
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            out.push_str(line.trim_start());
            out.push('\\n');
            continue;
        }
        // images: ![alt](url) -> drop; links: [text](url) -> text
        while let Some(start) = line.find("![") {
            let Some(end) = line[start..].find(")") else { break };
            line.replace_range(start..start + end + 1, "");
        }
        while let Some(start) = line.find('[') {
            let Some(mid) = line[start..].find("](") else { break };
            let Some(end) = line[start + mid..].find(')') else { break };
            let text = line[start + 1..start + mid].to_string();
            line.replace_range(start..start + mid + end + 1, &text);
        }
        let t = line.trim();
        if t.starts_with('#') {
            let stripped = t.trim_start_matches('#').trim();
            out.push_str(stripped);
        } else {
            out.push_str(t);
        }
        out.push('\\n');
        // collapse long code/blank stretches
        if out.len() > 24000 {
            out.push_str("…\\n(полное описание — по кнопке «Открыть сайт»)");
            break;
        }
    }
    out.trim().to_string()
}
'''

anchor = "fn sanitize(name: &str) -> String {"
assert anchor in src
src = src.replace(anchor, detail.strip() + "\n\n" + anchor, 1)
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

# ============ launch.rs: target addr Option
p = "src/launch.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace("pub fn launch(shared: &Arc<Shared>, cfg: &Config, id: &str, connect: bool) -> Result<PathBuf> {",
                  "pub fn launch(shared: &Arc<Shared>, cfg: &Config, id: &str, connect: Option<&str>) -> Result<PathBuf> {")
src = src.replace("    let args = build_args(shared, cfg, &resolved, connect)?;",
                  "    let args = build_args(shared, cfg, &resolved, connect.map(|s| s.to_string()))?;")
src = src.replace("fn build_args(shared: &Arc<Shared>, cfg: &Config, resolved: &Resolved, connect: bool) -> Result<Vec<String>> {",
                  "fn build_args(shared: &Arc<Shared>, cfg: &Config, resolved: &Resolved, connect: Option<String>) -> Result<Vec<String>> {")
src = src.replace('''    // The game joins the hosted server only when asked explicitly
    // (the «На сервер» button), never silently.
    if connect && shared.is_mc_running() && mc_release(&resolved.id) >= (1, 20) {
        let port = cfg.server_port;
        out.push("--quickPlayMultiplayer".into());
        out.push(format!("127.0.0.1:{port}"));
    }
    Ok(out)''', '''    // The game joins a server only when asked explicitly — the «На сервер»
    // button, a friend code, never silently.
    if let Some(addr) = connect {
        if mc_release(&resolved.id) >= (1, 20) {
            out.push("--quickPlayMultiplayer".into());
            out.push(addr);
        }
    }
    Ok(out)''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("detail+launch ok")
