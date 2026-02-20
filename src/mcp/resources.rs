use serde_json::json;
use std::path::PathBuf;

use super::protocol::{Resource, ResourceContent};
use crate::config::Config;
use crate::index;

pub fn list_resources(project_root: &PathBuf) -> Vec<Resource> {
    vec![Resource {
        uri: format!("booger://status/{}", project_root.display()),
        name: "Index Status".into(),
        description: format!(
            "Current index statistics for {}",
            project_root.display()
        ),
        mime_type: "application/json".into(),
    }]
}

pub fn read_resource(uri: &str, project_root: &PathBuf) -> Result<Vec<ResourceContent>, String> {
    let expected_uri = format!("booger://status/{}", project_root.display());
    if uri == expected_uri {
        let config = Config::load(project_root).unwrap_or_default();
        match index::index_status(project_root, &config) {
            Ok(stats) => {
                let body = json!({
                    "file_count": stats.file_count,
                    "chunk_count": stats.chunk_count,
                    "total_size_bytes": stats.total_size_bytes,
                    "db_size_bytes": stats.db_size_bytes,
                    "languages": stats.languages,
                });
                Ok(vec![ResourceContent {
                    uri: uri.to_string(),
                    mime_type: "application/json".into(),
                    text: serde_json::to_string_pretty(&body).unwrap_or_default(),
                }])
            }
            Err(e) => Err(format!("Failed to read status: {e}")),
        }
    } else {
        Err(format!("Unknown resource URI: {uri}"))
    }
}
