//! Mod loaders: which one a build needs, how its version id looks, and
//! which installed version satisfies it. Pure logic — no I/O here.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Loader {
    Vanilla,
    Forge,
    NeoForge,
    Fabric,
    Quilt,
}

impl Loader {
    /// Lenient parse of loader names as they appear in configs, mrpack
    /// dependency keys, CF manifests and Modrinth categories.
    pub fn parse(raw: &str) -> Loader {
        match raw.trim().to_ascii_lowercase().as_str() {
            "forge" => Loader::Forge,
            "neoforge" | "neo-forge" | "neoforged" => Loader::NeoForge,
            "fabric" | "fabric-loader" => Loader::Fabric,
            "quilt" | "quilt-loader" => Loader::Quilt,
            _ => Loader::Vanilla,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Loader::Vanilla => "vanilla",
            Loader::Forge => "forge",
            Loader::NeoForge => "neoforge",
            Loader::Fabric => "fabric",
            Loader::Quilt => "quilt",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Loader::Vanilla => "Vanilla",
            Loader::Forge => "Forge",
            Loader::NeoForge => "NeoForge",
            Loader::Fabric => "Fabric",
            Loader::Quilt => "Quilt",
        }
    }

    /// Forge-family loaders are installed by running the official installer.
    pub fn uses_installer(self) -> bool {
        matches!(self, Loader::Forge | Loader::NeoForge)
    }
}

/// Everything needed to run a build: Minecraft release, loader and its
/// exact version (empty = recommended/latest for that release).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub mc: String,
    pub loader: Loader,
    pub version: String,
}

impl Spec {
    pub fn new(mc: &str, loader: Loader, version: &str) -> Spec {
        Spec {
            mc: mc.trim().to_string(),
            loader,
            version: version.trim().to_string(),
        }
    }

    /// «Minecraft 1.20.1 · Forge 47.2.0» — shown on buttons and toasts.
    pub fn title(&self) -> String {
        match (self.loader, self.version.is_empty()) {
            (Loader::Vanilla, _) => format!("Minecraft {}", self.mc),
            (l, true) => format!("Minecraft {} · {}", self.mc, l.label()),
            (l, false) => format!("Minecraft {} · {} {}", self.mc, l.label(), self.version),
        }
    }
}

/// Reads the target from a Modrinth pack's `dependencies` map:
/// `{"minecraft": "1.20.1", "forge": "47.2.0"}` and friends.
pub fn mrpack_spec(deps: &HashMap<String, String>) -> Spec {
    let mc = deps.get("minecraft").cloned().unwrap_or_default();
    for (key, loader) in [
        ("neoforge", Loader::NeoForge),
        ("forge", Loader::Forge),
        ("fabric-loader", Loader::Fabric),
        ("quilt-loader", Loader::Quilt),
    ] {
        if let Some(v) = deps.get(key) {
            return Spec::new(&mc, loader, v);
        }
    }
    Spec::new(&mc, Loader::Vanilla, "")
}

/// Reads the target from a CurseForge manifest: `minecraft.version` plus
/// `modLoaders[].id` such as `forge-47.2.0` or `fabric-0.15.3`.
pub fn cf_spec(mc: &str, loaders: &[(String, bool)]) -> Spec {
    let chosen = loaders
        .iter()
        .find(|(_, primary)| *primary)
        .or_else(|| loaders.first())
        .map(|(id, _)| id.as_str())
        .unwrap_or("");
    let (name, version) = chosen.split_once('-').unwrap_or((chosen, ""));
    let loader = Loader::parse(name);
    // CF writes 1.20.1 NeoForge as `neoforge-1.20.1-47.1.106` on some packs.
    let version = version
        .strip_prefix(&format!("{mc}-"))
        .unwrap_or(version);
    if loader == Loader::Vanilla {
        Spec::new(mc, Loader::Vanilla, "")
    } else {
        Spec::new(mc, loader, version)
    }
}

/// NeoForge versions encode the Minecraft release: MC 1.20.4 → `20.4.x`,
/// MC 1.21 → `21.0.x`. MC 1.20.1 NeoForge is published as a Forge fork.
pub fn neoforge_prefix(mc: &str) -> Option<String> {
    let rest = mc.strip_prefix("1.")?;
    let mut parts = rest.split('.');
    let minor: u32 = parts.next()?.parse().ok()?;
    let patch: u32 = parts.next().map(|p| p.parse().ok()).unwrap_or(Some(0))?;
    if minor < 20 || (minor == 20 && patch <= 1) {
        return None;
    }
    Some(format!("{minor}.{patch}."))
}

/// Minecraft release from a NeoForge version: `20.4.237` → `1.20.4`,
/// `21.0.10` → `1.21`.
pub fn mc_from_neoforge(version: &str) -> String {
    let mut parts = version.split('.');
    let minor = parts.next().unwrap_or("");
    let patch = parts.next().unwrap_or("0");
    if patch == "0" {
        format!("1.{minor}")
    } else {
        format!("1.{minor}.{patch}")
    }
}

/// Loader kind, Minecraft release and loader version guessed from a
/// version folder name (used when no madrid.json marker exists).
pub fn parse_version_id(id: &str) -> (Loader, String, String) {
    if let Some(rest) = id.strip_prefix("fabric-loader-") {
        if let Some((lv, mc)) = rest.split_once('-') {
            return (Loader::Fabric, mc.to_string(), lv.to_string());
        }
    }
    if let Some(rest) = id.strip_prefix("quilt-loader-") {
        if let Some((lv, mc)) = rest.split_once('-') {
            return (Loader::Quilt, mc.to_string(), lv.to_string());
        }
    }
    if let Some(v) = id.strip_prefix("neoforge-") {
        return (Loader::NeoForge, mc_from_neoforge(v), v.to_string());
    }
    if let Some((mc, v)) = id.split_once("-forge-") {
        // Legacy ids look like `1.12.2-forge-14.23.5.2859` or
        // `1.7.10-Forge10.13.4.1614-1.7.10`; both keep the release first.
        return (Loader::Forge, mc.to_string(), v.to_string());
    }
    if let Some((mc, v)) = id.split_once("-forge") {
        return (Loader::Forge, mc.to_string(), v.trim_start_matches('-').to_string());
    }
    (Loader::Vanilla, id.to_string(), String::new())
}

/// One installed version as the launcher sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installed {
    pub id: String,
    pub mc: String,
    pub loader: Loader,
    pub version: String,
}

/// The installed version that runs `spec`, if any. An exact loader version
/// is honoured when the spec names one; otherwise any build of that loader
/// for the release fits.
pub fn pick_installed(list: &[Installed], spec: &Spec) -> Option<String> {
    let fits = |v: &&Installed| v.mc == spec.mc && v.loader == spec.loader;
    if spec.loader == Loader::Vanilla || spec.version.is_empty() {
        return list.iter().find(fits).map(|v| v.id.clone());
    }
    list.iter()
        .filter(fits)
        .find(|v| v.version == spec.version)
        .map(|v| v.id.clone())
}

/// Whether a found Java can run a build. Forge-family and old releases
/// break on newer Java, so they want the exact major; others take newer.
pub fn java_fits(found: u32, required: u32, loader: Loader) -> bool {
    if found < required {
        return false;
    }
    if required <= 8 || loader.uses_installer() {
        return found == required;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deps(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn mrpack_reads_every_loader() {
        let s = mrpack_spec(&deps(&[("minecraft", "1.20.1"), ("forge", "47.2.0")]));
        assert_eq!(s, Spec::new("1.20.1", Loader::Forge, "47.2.0"));
        let s = mrpack_spec(&deps(&[("minecraft", "1.20.1"), ("fabric-loader", "0.15.3")]));
        assert_eq!(s, Spec::new("1.20.1", Loader::Fabric, "0.15.3"));
        let s = mrpack_spec(&deps(&[("minecraft", "1.21.1"), ("neoforge", "21.1.77")]));
        assert_eq!(s, Spec::new("1.21.1", Loader::NeoForge, "21.1.77"));
        let s = mrpack_spec(&deps(&[("minecraft", "1.19.2"), ("quilt-loader", "0.20.0")]));
        assert_eq!(s, Spec::new("1.19.2", Loader::Quilt, "0.20.0"));
        let s = mrpack_spec(&deps(&[("minecraft", "1.21")]));
        assert_eq!(s, Spec::new("1.21", Loader::Vanilla, ""));
    }

    #[test]
    fn cf_manifest_loader_ids() {
        let primary = |id: &str| vec![(id.to_string(), true)];
        assert_eq!(
            cf_spec("1.20.1", &primary("forge-47.2.0")),
            Spec::new("1.20.1", Loader::Forge, "47.2.0")
        );
        assert_eq!(
            cf_spec("1.20.1", &primary("fabric-0.15.3")),
            Spec::new("1.20.1", Loader::Fabric, "0.15.3")
        );
        assert_eq!(
            cf_spec("1.20.1", &primary("neoforge-1.20.1-47.1.106")),
            Spec::new("1.20.1", Loader::NeoForge, "47.1.106")
        );
        assert_eq!(
            cf_spec("1.21.1", &primary("neoforge-21.1.77")),
            Spec::new("1.21.1", Loader::NeoForge, "21.1.77")
        );
        // primary wins over order
        let list = vec![
            ("fabric-0.14.0".to_string(), false),
            ("forge-14.23.5.2859".to_string(), true),
        ];
        assert_eq!(
            cf_spec("1.12.2", &list),
            Spec::new("1.12.2", Loader::Forge, "14.23.5.2859")
        );
        assert_eq!(cf_spec("1.16.5", &[]), Spec::new("1.16.5", Loader::Vanilla, ""));
    }

    #[test]
    fn neoforge_prefix_maps_releases() {
        assert_eq!(neoforge_prefix("1.20.4").as_deref(), Some("20.4."));
        assert_eq!(neoforge_prefix("1.21").as_deref(), Some("21.0."));
        assert_eq!(neoforge_prefix("1.21.1").as_deref(), Some("21.1."));
        assert_eq!(neoforge_prefix("1.20.1"), None);
        assert_eq!(neoforge_prefix("1.19.2"), None);
        assert_eq!(mc_from_neoforge("20.4.237"), "1.20.4");
        assert_eq!(mc_from_neoforge("21.0.10"), "1.21");
    }

    #[test]
    fn version_ids_parse_back() {
        assert_eq!(
            parse_version_id("1.20.1-forge-47.4.28"),
            (Loader::Forge, "1.20.1".into(), "47.4.28".into())
        );
        assert_eq!(
            parse_version_id("fabric-loader-0.15.3-1.20.1"),
            (Loader::Fabric, "1.20.1".into(), "0.15.3".into())
        );
        assert_eq!(
            parse_version_id("quilt-loader-0.20.0-1.19.2"),
            (Loader::Quilt, "1.19.2".into(), "0.20.0".into())
        );
        assert_eq!(
            parse_version_id("neoforge-21.1.77"),
            (Loader::NeoForge, "1.21.1".into(), "21.1.77".into())
        );
        assert_eq!(
            parse_version_id("1.20.1"),
            (Loader::Vanilla, "1.20.1".into(), String::new())
        );
    }

    #[test]
    fn picks_exact_loader_version_when_asked() {
        let list = vec![
            Installed {
                id: "1.20.1".into(),
                mc: "1.20.1".into(),
                loader: Loader::Vanilla,
                version: String::new(),
            },
            Installed {
                id: "1.20.1-forge-47.4.28".into(),
                mc: "1.20.1".into(),
                loader: Loader::Forge,
                version: "47.4.28".into(),
            },
        ];
        let any_forge = Spec::new("1.20.1", Loader::Forge, "");
        assert_eq!(pick_installed(&list, &any_forge).as_deref(), Some("1.20.1-forge-47.4.28"));
        let exact = Spec::new("1.20.1", Loader::Forge, "47.2.0");
        assert_eq!(pick_installed(&list, &exact), None);
        let vanilla = Spec::new("1.20.1", Loader::Vanilla, "");
        assert_eq!(pick_installed(&list, &vanilla).as_deref(), Some("1.20.1"));
        let fabric = Spec::new("1.20.1", Loader::Fabric, "");
        assert_eq!(pick_installed(&list, &fabric), None);
    }

    #[test]
    fn java_rules() {
        assert!(java_fits(17, 17, Loader::Forge));
        assert!(!java_fits(21, 17, Loader::Forge));
        assert!(java_fits(21, 17, Loader::Fabric));
        assert!(!java_fits(17, 8, Loader::Vanilla));
        assert!(java_fits(8, 8, Loader::Forge));
        assert!(!java_fits(8, 17, Loader::Fabric));
    }

    #[test]
    fn loader_parse_is_lenient() {
        assert_eq!(Loader::parse("Fabric-Loader"), Loader::Fabric);
        assert_eq!(Loader::parse("NeoForge"), Loader::NeoForge);
        assert_eq!(Loader::parse(""), Loader::Vanilla);
        assert_eq!(Spec::new("1.20.1", Loader::Forge, "47.2.0").title(), "Minecraft 1.20.1 · Forge 47.2.0");
    }
}
