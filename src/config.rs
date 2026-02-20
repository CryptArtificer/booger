use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub storage: StorageConfig,
    pub resources: ResourceConfig,
    pub embed: EmbedConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Where the .booger index directory lives (default: inside indexed dir)
    pub path: Option<PathBuf>,
    /// Max total index size in bytes (0 = unlimited)
    pub max_size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceConfig {
    /// Max threads for parallel file walking/processing (0 = half available cores)
    pub max_threads: usize,
    /// Max memory budget hint in bytes for batching (0 = 256MB default)
    pub max_memory_bytes: u64,
    /// Max files to process per indexing batch before committing
    pub batch_size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmbedConfig {
    pub backend: EmbedBackend,
    /// Max concurrent embedding requests
    pub max_concurrent: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EmbedBackend {
    #[serde(rename = "ollama")]
    Ollama { model: String, url: String },
    #[serde(rename = "openai")]
    OpenAi { model: String },
    #[serde(rename = "none")]
    None,
}

impl Default for Config {
    fn default() -> Self {
        let num_cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        Self {
            storage: StorageConfig {
                path: None,
                max_size_bytes: 0,
            },
            resources: ResourceConfig {
                max_threads: (num_cpus / 2).max(1),
                max_memory_bytes: 256 * 1024 * 1024, // 256MB
                batch_size: 500,
            },
            embed: EmbedConfig {
                backend: EmbedBackend::None,
                max_concurrent: 4,
            },
        }
    }
}

impl Config {
    /// Load config from a .booger/config.toml file, falling back to defaults.
    pub fn load(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join(".booger").join("config.toml");
        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)
                .with_context(|| format!("reading config from {}", config_path.display()))?;
            toml::from_str(&contents)
                .with_context(|| format!("parsing config from {}", config_path.display()))
        } else {
            Ok(Self::default())
        }
    }

    /// Resolve the actual storage directory path.
    pub fn storage_dir(&self, project_root: &Path) -> PathBuf {
        self.storage
            .path
            .clone()
            .unwrap_or_else(|| project_root.join(".booger"))
    }

    /// Effective thread count, resolving 0 to a sensible default.
    pub fn effective_threads(&self) -> usize {
        if self.resources.max_threads == 0 {
            let num_cpus = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4);
            (num_cpus / 2).max(1)
        } else {
            self.resources.max_threads
        }
    }

    /// Write current config to disk (for `booger init`).
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = self.storage_dir(project_root);
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("creating storage dir {}", dir.display()))?;
        let config_path = dir.join("config.toml");
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, contents)
            .with_context(|| format!("writing config to {}", config_path.display()))?;
        Ok(())
    }
}

// ── Global project registry ──
// Lives at ~/.booger/projects.json — maps short names to directory paths.

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ProjectRegistry {
    pub projects: BTreeMap<String, ProjectEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub path: PathBuf,
}

impl ProjectRegistry {
    fn registry_path() -> PathBuf {
        dirs_path().join("projects.json")
    }

    pub fn load() -> Result<Self> {
        let path = Self::registry_path();
        if path.exists() {
            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("reading registry from {}", path.display()))?;
            serde_json::from_str(&contents)
                .with_context(|| format!("parsing registry from {}", path.display()))
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::registry_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, contents)
            .with_context(|| format!("writing registry to {}", path.display()))?;
        Ok(())
    }

    pub fn add(&mut self, name: String, path: PathBuf) {
        self.projects.insert(name, ProjectEntry { path });
    }

    pub fn remove(&mut self, name: &str) -> bool {
        self.projects.remove(name).is_some()
    }

    /// Resolve a project name or path to an actual directory.
    /// Tries: registered project name first, then treats it as a literal path.
    pub fn resolve(&self, name_or_path: &str) -> Option<PathBuf> {
        if let Some(entry) = self.projects.get(name_or_path) {
            Some(entry.path.clone())
        } else {
            let p = PathBuf::from(name_or_path);
            if p.is_dir() {
                Some(p)
            } else {
                None
            }
        }
    }
}

fn dirs_path() -> PathBuf {
    dirs_home().join(".booger")
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}
