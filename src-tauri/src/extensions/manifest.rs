use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ExtensionManifest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(skip)]
    pub path: PathBuf,
}

pub fn discover(workspace: &Path) -> Vec<ExtensionManifest> {
    let mut roots = manifest_roots(workspace);
    roots.dedup();
    let mut manifests = Vec::new();
    for root in roots {
        collect_manifests(&root, &mut manifests);
    }
    dedupe(manifests)
}

fn manifest_roots(workspace: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".sandevistan").join("extensions"));
    }
    for ancestor in workspace.ancestors() {
        roots.push(ancestor.join(".sandevistan").join("extensions"));
        if ancestor.join(".git").is_dir() {
            break;
        }
    }
    roots
}

fn collect_manifests(root: &Path, manifests: &mut Vec<ExtensionManifest>) {
    if !root.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let manifest_path = if path.is_dir() {
            path.join("extension.toml")
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
            path
        } else {
            continue;
        };
        if let Some(manifest) = parse_manifest(&manifest_path) {
            manifests.push(manifest);
        }
    }
}

fn parse_manifest(path: &Path) -> Option<ExtensionManifest> {
    let content = fs::read_to_string(path).ok()?;
    let mut manifest = toml::from_str::<ExtensionManifest>(&content).ok()?;
    if !valid_id(&manifest.id) {
        return None;
    }
    manifest.path = path.to_path_buf();
    Some(manifest)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
}

fn dedupe(manifests: Vec<ExtensionManifest>) -> Vec<ExtensionManifest> {
    let mut seen = std::collections::HashSet::new();
    manifests
        .into_iter()
        .filter(|manifest| seen.insert(manifest.id.clone()))
        .collect()
}
