use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeatureDomain {
    Pilcrow,
    Silcrow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeatureStatus {
    Stable,
    Experimental,
    Planned,
    Deprecated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub id: String,
    pub name: String,
    pub domain: FeatureDomain,
    pub status: FeatureStatus,
    pub summary: String,
    pub spec: String,
    #[serde(default)]
    pub validation_rules: Vec<String>,
    #[serde(default)]
    pub scaffold_templates: Vec<String>,
    #[serde(default)]
    pub canonical_usage: Option<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub invalid_examples: Vec<String>,
    #[serde(default)]
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub test_refs: Vec<String>,
    #[serde(default)]
    pub silcrow_boundary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub registry_schema_version: u32,
    pub framework_version: String,
    #[serde(default)]
    pub features: Vec<Feature>,
}

impl Registry {
    pub fn load(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)
            .with_context(|| format!("failed to read registry at {}", path.display()))?;
        toml::from_str(&source).with_context(|| format!("failed to parse {}", path.display()))
    }

    pub fn load_from_project(project_root: &Path) -> Result<Self> {
        Self::load(&project_root.join("registry.toml"))
    }

    pub fn feature(&self, id: &str) -> Option<&Feature> {
        self.features.iter().find(|feature| feature.id == id)
    }

    pub fn filtered(
        &self,
        status: Option<FeatureStatus>,
        domain: Option<FeatureDomain>,
    ) -> Vec<&Feature> {
        self.features
            .iter()
            .filter(|feature| status.map_or(true, |status| feature.status == status))
            .filter(|feature| domain.map_or(true, |domain| feature.domain == domain))
            .collect()
    }
}

fn is_project_root(path: &Path) -> bool {
    path.join("Cargo.toml").exists()
        && path.join("crates").is_dir()
        && path.join("registry.toml").exists()
}

fn find_project_root_up(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    if dir.is_file() {
        dir.pop();
    }
    loop {
        if is_project_root(&dir) {
            return Some(dir.clone());
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn find_project_root_from_env() -> Option<PathBuf> {
    std::env::var("PILCROW_PROJECT_ROOT")
        .ok()
        .map(PathBuf::from)
        .and_then(|path| find_project_root_up(&path))
}

fn find_project_root_from_exe() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| find_project_root_up(&exe))
}

pub fn find_project_root(start: impl AsRef<Path>) -> Option<PathBuf> {
    find_project_root_up(start.as_ref())
        .or_else(find_project_root_from_env)
        .or_else(find_project_root_from_exe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registry_and_filters_status() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let registry = Registry::load_from_project(&root).unwrap();
        assert!(registry.feature("ssr-pages").is_some());
        // All gaps are closed — no planned features remain
        assert!(registry
            .filtered(Some(FeatureStatus::Planned), None)
            .is_empty());
        // Core features are stable
        assert!(registry
            .feature("islands")
            .map_or(false, |f| f.status == FeatureStatus::Stable));
        assert!(registry
            .feature("head-meta")
            .map_or(false, |f| f.status == FeatureStatus::Stable));
    }
}
