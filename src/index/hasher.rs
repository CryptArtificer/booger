use anyhow::Result;
use std::path::Path;

/// Hash file contents using BLAKE3. Fast and collision-resistant.
/// Reads the file in streaming fashion to avoid loading large files into memory.
pub fn hash_file(path: &Path) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::with_capacity(64 * 1024, file);
    std::io::copy(&mut reader, &mut hasher)?;
    Ok(hasher.finalize().to_hex().to_string())
}

/// Hash a byte slice directly (for in-memory content).
pub fn hash_bytes(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}
