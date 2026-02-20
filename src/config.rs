/// Booger configuration, loaded from .booger/config.toml or defaults.
pub struct Config {
    pub storage_path: std::path::PathBuf,
    pub embed_backend: EmbedBackend,
}

pub enum EmbedBackend {
    Ollama { model: String, url: String },
    OpenAi { model: String },
}

impl Default for Config {
    fn default() -> Self {
        Self {
            storage_path: std::path::PathBuf::from(".booger"),
            embed_backend: EmbedBackend::Ollama {
                model: "nomic-embed-text".into(),
                url: "http://localhost:11434".into(),
            },
        }
    }
}
