use std::fmt;
use std::path::{Component, Path, PathBuf};

pub const DEFAULT_SCENE_ID: &str = "demo-paths";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SceneEntry {
    id: &'static str,
    relative_path: &'static str,
}

macro_rules! scene_entry {
    ($id:literal) => {
        SceneEntry {
            id: $id,
            relative_path: concat!("games/", $id, ".json"),
        }
    };
}

const SCENE_ENTRIES: &[SceneEntry] = &[
    scene_entry!("demo-paths"),
    scene_entry!("metro-pulse"),
    scene_entry!("cargo-loom"),
    scene_entry!("factory-line-seeds"),
    scene_entry!("garden-pollinators"),
    scene_entry!("data-packet-city"),
    scene_entry!("emergency-dispatch"),
    scene_entry!("power-grid-balancer"),
    scene_entry!("river-ferries"),
    scene_entry!("night-market-runners"),
    scene_entry!("orbital-transfers"),
    scene_entry!("gpu-launch-week"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneRef {
    pub scene_id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SceneRegistry {
    project_root: PathBuf,
    resource_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneRegistryError {
    UnknownSceneId(String),
    UnsafeRegistryPath {
        scene_id: &'static str,
        relative_path: &'static str,
    },
}

impl fmt::Display for SceneRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSceneId(scene_id) => write!(f, "unknown scene_id: {scene_id}"),
            Self::UnsafeRegistryPath {
                scene_id,
                relative_path,
            } => write!(
                f,
                "unsafe registry path for scene_id {scene_id}: {relative_path}"
            ),
        }
    }
}

impl std::error::Error for SceneRegistryError {}

impl SceneRegistry {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            resource_root: None,
        }
    }

    pub fn with_resource_root(project_root: PathBuf, resource_root: Option<PathBuf>) -> Self {
        let mut registry = Self::new(project_root);
        registry.resource_root = resource_root;
        registry
    }

    pub fn resolve(&self, scene_id: &str) -> Result<SceneRef, SceneRegistryError> {
        let entry = SCENE_ENTRIES
            .iter()
            .find(|entry| entry.id == scene_id)
            .ok_or_else(|| SceneRegistryError::UnknownSceneId(scene_id.to_string()))?;

        let relative = safe_games_json_path(entry.relative_path).ok_or(
            SceneRegistryError::UnsafeRegistryPath {
                scene_id: entry.id,
                relative_path: entry.relative_path,
            },
        )?;

        let resource_path = self
            .resource_root
            .as_ref()
            .map(|root| root.join(&relative))
            .filter(|path| path.is_file());
        let path = resource_path.unwrap_or_else(|| self.project_root.join(relative));

        Ok(SceneRef {
            scene_id: entry.id.to_string(),
            path,
        })
    }

    pub fn default_scene(&self) -> Result<SceneRef, SceneRegistryError> {
        self.resolve(DEFAULT_SCENE_ID)
    }
}

fn safe_games_json_path(path: &'static str) -> Option<PathBuf> {
    let path = Path::new(path);
    if path.is_absolute() || path.extension().and_then(|ext| ext.to_str()) != Some("json") {
        return None;
    }

    let mut components = path.components();
    match components.next() {
        Some(Component::Normal(first)) if first == "games" => {}
        _ => return None,
    }

    for component in components {
        match component {
            Component::Normal(_) => {}
            _ => return None,
        }
    }

    Some(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_registered_scene_to_games_json() {
        let root = PathBuf::from("/repo");
        let registry = SceneRegistry::new(root.clone());

        let scene = registry.resolve("demo-paths").expect("registered scene");

        assert_eq!(scene.scene_id, "demo-paths");
        assert_eq!(scene.path, root.join("games/demo-paths.json"));
    }

    #[test]
    fn rejects_unknown_scene_ids_before_resolving_a_path() {
        let registry = SceneRegistry::new(PathBuf::from("/repo"));

        let err = registry
            .resolve("../games/demo-paths.json")
            .expect_err("path-like scene ids are not registered");

        assert_eq!(
            err,
            SceneRegistryError::UnknownSceneId("../games/demo-paths.json".to_string())
        );
    }

    #[test]
    fn registry_paths_must_stay_inside_games_json() {
        assert!(safe_games_json_path("games/demo-paths.json").is_some());
        assert!(safe_games_json_path("games/nested/demo-paths.json").is_some());
        assert!(safe_games_json_path("../games/demo-paths.json").is_none());
        assert!(safe_games_json_path("games/../demo-paths.json").is_none());
        assert!(safe_games_json_path("docs/demo-paths.json").is_none());
        assert!(safe_games_json_path("games/demo-paths.toml").is_none());
    }
}
