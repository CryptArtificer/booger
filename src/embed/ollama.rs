use anyhow::{bail, Context, Result};
use serde::Deserialize;

use super::{Embedder, Embedding};

pub struct OllamaEmbedder {
    base_url: String,
    model: String,
    dimensions: usize,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

impl OllamaEmbedder {
    pub fn new(base_url: &str, model: &str) -> Result<Self> {
        let mut embedder = Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            dimensions: 0,
        };

        // Probe dimensions with a short test string
        let test = embedder.embed("test")?;
        embedder.dimensions = test.len();
        Ok(embedder)
    }

    pub fn default() -> Result<Self> {
        Self::new("http://localhost:11434", "nomic-embed-text")
    }
}

impl Embedder for OllamaEmbedder {
    fn embed(&self, text: &str) -> Result<Embedding> {
        let text = if text.is_empty() { " " } else { text };
        // Truncate very long texts to avoid overwhelming the model
        let text = if text.len() > 8192 { &text[..8192] } else { text };

        let url = format!("{}/api/embeddings", self.base_url);
        let body = serde_json::json!({
            "model": self.model,
            "prompt": text,
        });

        let result = ureq::post(&url).send_json(&body);

        let mut response = match result {
            Ok(r) => r,
            Err(ureq::Error::StatusCode(code)) => {
                bail!("ollama returned HTTP {code}");
            }
            Err(e) => {
                return Err(anyhow::anyhow!(e).context("ollama embedding request failed"));
            }
        };

        let resp: EmbeddingResponse = response
            .body_mut()
            .read_json()
            .context("parsing ollama response")?;

        Ok(resp.embedding)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}
