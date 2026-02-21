use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;

use super::schema;

pub struct Store {
    conn: Connection,
}

/// A search result returned from FTS queries.
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub file_path: String,
    pub language: Option<String>,
    pub chunk_kind: String,
    pub chunk_name: Option<String>,
    pub signature: Option<String>,
    pub start_line: i64,
    pub end_line: i64,
    pub content: String,
    pub rank: f64,
}

/// A file record as stored in the index.
pub struct FileRecord {
    pub id: i64,
    pub path: String,
    pub content_hash: String,
    pub size_bytes: i64,
    pub language: Option<String>,
    pub indexed_at: String,
}

/// A chunk record for insertion.
pub struct ChunkInsert {
    pub kind: String,
    pub name: Option<String>,
    pub content: String,
    pub signature: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Debug, Serialize)]
pub struct Annotation {
    pub id: i64,
    pub target: String,
    pub note: String,
    pub session_id: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WorksetEntry {
    pub id: i64,
    pub path: String,
    pub kind: String,
    pub session_id: Option<String>,
    pub created_at: String,
}

pub struct IndexStats {
    pub file_count: i64,
    pub chunk_count: i64,
    pub total_size_bytes: i64,
    pub db_size_bytes: u64,
    pub languages: Vec<(String, i64)>,
}

impl IndexStats {
    pub fn empty() -> Self {
        Self {
            file_count: 0,
            chunk_count: 0,
            total_size_bytes: 0,
            db_size_bytes: 0,
            languages: Vec::new(),
        }
    }
}

impl Store {
    /// Open (or create) the database, running migrations. Use for write paths.
    pub fn open(storage_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(storage_dir)
            .with_context(|| format!("creating storage dir {}", storage_dir.display()))?;
        let db_path = storage_dir.join("index.db");
        let conn = Connection::open(&db_path)
            .with_context(|| format!("opening database at {}", db_path.display()))?;
        schema::run_migrations(&conn)?;
        Ok(Self { conn })
    }

    /// Open the database only if it already exists. Returns None otherwise.
    /// Use for read-only paths to avoid creating empty databases as a side effect.
    pub fn open_if_exists(storage_dir: &Path) -> Result<Option<Self>> {
        let db_path = storage_dir.join("index.db");
        if !db_path.exists() {
            return Ok(None);
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("opening database at {}", db_path.display()))?;
        schema::run_migrations(&conn)?;
        Ok(Some(Self { conn }))
    }

    /// Look up a file by path. Returns None if not indexed.
    pub fn get_file(&self, path: &str) -> Result<Option<FileRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, content_hash, size_bytes, language, indexed_at FROM files WHERE path = ?1",
        )?;
        let mut rows = stmt.query_map(params![path], |row| {
            Ok(FileRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                content_hash: row.get(2)?,
                size_bytes: row.get(3)?,
                language: row.get(4)?,
                indexed_at: row.get(5)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// Insert or update a file record. Returns the file id.
    /// Deletes old chunks on update (CASCADE handles this).
    pub fn upsert_file(
        &self,
        path: &str,
        content_hash: &str,
        size_bytes: i64,
        language: Option<&str>,
    ) -> Result<i64> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO files (path, content_hash, size_bytes, language, indexed_at, mtime)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL)
             ON CONFLICT(path) DO UPDATE SET
                content_hash = excluded.content_hash,
                size_bytes = excluded.size_bytes,
                language = excluded.language,
                indexed_at = excluded.indexed_at",
            params![path, content_hash, size_bytes, language, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Bulk insert chunks for a file. Call within a transaction for performance.
    pub fn insert_chunks(&self, file_id: i64, chunks: &[ChunkInsert]) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO chunks (file_id, kind, name, content, signature, start_line, end_line, start_byte, end_byte)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for chunk in chunks {
            stmt.execute(params![
                file_id,
                chunk.kind,
                chunk.name,
                chunk.content,
                chunk.signature,
                chunk.start_line,
                chunk.end_line,
                chunk.start_byte,
                chunk.end_byte,
            ])?;
        }
        Ok(())
    }

    /// Delete all chunks for a file (used before re-indexing).
    pub fn delete_chunks_for_file(&self, file_id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM chunks WHERE file_id = ?1", params![file_id])?;
        Ok(())
    }

    /// Remove a file and its chunks from the index.
    pub fn remove_file(&self, path: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM files WHERE path = ?1", params![path])?;
        Ok(())
    }

    /// Begin a transaction. Returns the inner connection for batch ops.
    pub fn begin_transaction(&self) -> Result<()> {
        self.conn.execute_batch("BEGIN TRANSACTION")?;
        Ok(())
    }

    pub fn commit_transaction(&self) -> Result<()> {
        self.conn.execute_batch("COMMIT")?;
        Ok(())
    }

    pub fn rollback_transaction(&self) -> Result<()> {
        self.conn.execute_batch("ROLLBACK")?;
        Ok(())
    }

    /// Full-text search over indexed chunks.
    pub fn search(
        &self,
        query: &str,
        language: Option<&str>,
        path_prefix: Option<&str>,
        kind: Option<&str>,
        max_results: usize,
    ) -> Result<Vec<SearchResult>> {
        let query = sanitize_fts_query(query);
        let query = query.as_str();

        let mut sql = String::from(
            "SELECT f.path, f.language, c.kind, c.name, c.signature, c.start_line, c.end_line, c.content,
                    chunks_fts.rank
             FROM chunks_fts
             JOIN chunks c ON c.id = chunks_fts.rowid
             JOIN files f ON f.id = c.file_id
             WHERE chunks_fts MATCH ?1",
        );
        let mut param_idx = 2;

        if language.is_some() {
            sql.push_str(&format!(" AND f.language = ?{param_idx}"));
            param_idx += 1;
        }
        if path_prefix.is_some() {
            sql.push_str(&format!(" AND f.path LIKE ?{param_idx} || '%'"));
            param_idx += 1;
        }
        if kind.is_some() {
            sql.push_str(&format!(" AND c.kind = ?{param_idx}"));
            param_idx += 1;
        }

        sql.push_str(&format!(" ORDER BY chunks_fts.rank LIMIT ?{param_idx}"));

        let mut stmt = self.conn.prepare(&sql)?;

        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        params_vec.push(Box::new(query.to_string()));
        if let Some(lang) = language {
            params_vec.push(Box::new(lang.to_string()));
        }
        if let Some(prefix) = path_prefix {
            params_vec.push(Box::new(prefix.to_string()));
        }
        if let Some(k) = kind {
            params_vec.push(Box::new(k.to_string()));
        }
        params_vec.push(Box::new(max_results as i64));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(SearchResult {
                file_path: row.get(0)?,
                language: row.get(1)?,
                chunk_kind: row.get(2)?,
                chunk_name: row.get(3)?,
                signature: row.get(4)?,
                start_line: row.get(5)?,
                end_line: row.get(6)?,
                content: row.get(7)?,
                rank: row.get(8)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// List all symbols (chunks) in a file or directory, optionally filtered by kind.
    pub fn list_symbols(
        &self,
        path_prefix: Option<&str>,
        kind: Option<&str>,
    ) -> Result<Vec<SearchResult>> {
        let mut sql = String::from(
            "SELECT f.path, f.language, c.kind, c.name, c.signature, c.start_line, c.end_line, c.content
             FROM chunks c
             JOIN files f ON f.id = c.file_id
             WHERE c.kind != 'raw'",
        );
        let mut param_idx = 1;

        if path_prefix.is_some() {
            sql.push_str(&format!(" AND f.path LIKE ?{param_idx} || '%'"));
            param_idx += 1;
        }
        if kind.is_some() {
            sql.push_str(&format!(" AND c.kind = ?{param_idx}"));
        }

        sql.push_str(" ORDER BY f.path, c.start_line");

        let mut stmt = self.conn.prepare(&sql)?;

        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(prefix) = path_prefix {
            params_vec.push(Box::new(prefix.to_string()));
        }
        if let Some(k) = kind {
            params_vec.push(Box::new(k.to_string()));
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(SearchResult {
                file_path: row.get(0)?,
                language: row.get(1)?,
                chunk_kind: row.get(2)?,
                chunk_name: row.get(3)?,
                signature: row.get(4)?,
                start_line: row.get(5)?,
                end_line: row.get(6)?,
                content: row.get(7)?,
                rank: 0.0,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Return all chunks, optionally filtered by path and/or kind.
    /// Unlike `list_symbols`, this includes raw chunks.
    pub fn all_chunks(
        &self,
        path_prefix: Option<&str>,
        kind: Option<&str>,
    ) -> Result<Vec<SearchResult>> {
        let mut sql = String::from(
            "SELECT f.path, f.language, c.kind, c.name, c.signature, c.start_line, c.end_line, c.content
             FROM chunks c
             JOIN files f ON f.id = c.file_id
             WHERE 1=1",
        );
        let mut param_idx = 1;

        if path_prefix.is_some() {
            sql.push_str(&format!(" AND f.path LIKE ?{param_idx} || '%'"));
            param_idx += 1;
        }
        if kind.is_some() {
            sql.push_str(&format!(" AND c.kind = ?{param_idx}"));
        }

        sql.push_str(" ORDER BY f.path, c.start_line");

        let mut stmt = self.conn.prepare(&sql)?;

        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(prefix) = path_prefix {
            params_vec.push(Box::new(prefix.to_string()));
        }
        if let Some(k) = kind {
            params_vec.push(Box::new(k.to_string()));
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(SearchResult {
                file_path: row.get(0)?,
                language: row.get(1)?,
                chunk_kind: row.get(2)?,
                chunk_name: row.get(3)?,
                signature: row.get(4)?,
                start_line: row.get(5)?,
                end_line: row.get(6)?,
                content: row.get(7)?,
                rank: 0.0,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Return chunks from files indexed after a given timestamp.
    pub fn chunks_changed_since(
        &self,
        since: &str,
        kind: Option<&str>,
    ) -> Result<Vec<SearchResult>> {
        let mut sql = String::from(
            "SELECT f.path, f.language, c.kind, c.name, c.signature, c.start_line, c.end_line, c.content
             FROM chunks c
             JOIN files f ON f.id = c.file_id
             WHERE f.indexed_at > ?1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        params_vec.push(Box::new(since.to_string()));

        if let Some(k) = kind {
            sql.push_str(" AND c.kind = ?2");
            params_vec.push(Box::new(k.to_string()));
        }

        sql.push_str(" ORDER BY f.indexed_at DESC, f.path, c.start_line");

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(SearchResult {
                file_path: row.get(0)?,
                language: row.get(1)?,
                chunk_kind: row.get(2)?,
                chunk_name: row.get(3)?,
                signature: row.get(4)?,
                start_line: row.get(5)?,
                end_line: row.get(6)?,
                content: row.get(7)?,
                rank: 0.0,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Return a breakdown of chunks by kind.
    pub fn kind_stats(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, COUNT(*) FROM chunks GROUP BY kind ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    // ── Annotations ──

    pub fn add_annotation(
        &self,
        target: &str,
        note: &str,
        session_id: Option<&str>,
        ttl_seconds: Option<i64>,
    ) -> Result<i64> {
        let now = chrono::Utc::now();
        let created_at = now.to_rfc3339();
        let expires_at = ttl_seconds.map(|s| {
            (now + chrono::Duration::seconds(s)).to_rfc3339()
        });
        self.conn.execute(
            "INSERT INTO annotations (target, note, session_id, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![target, note, session_id, created_at, expires_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_annotations(
        &self,
        target: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<Vec<Annotation>> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut sql = String::from(
            "SELECT id, target, note, session_id, created_at, expires_at FROM annotations
             WHERE (expires_at IS NULL OR expires_at > ?1)",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        params_vec.push(Box::new(now));

        if let Some(t) = target {
            sql.push_str(" AND target = ?2");
            params_vec.push(Box::new(t.to_string()));
            if let Some(s) = session_id {
                sql.push_str(" AND (session_id IS NULL OR session_id = ?3)");
                params_vec.push(Box::new(s.to_string()));
            }
        } else if let Some(s) = session_id {
            sql.push_str(" AND (session_id IS NULL OR session_id = ?2)");
            params_vec.push(Box::new(s.to_string()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(Annotation {
                id: row.get(0)?,
                target: row.get(1)?,
                note: row.get(2)?,
                session_id: row.get(3)?,
                created_at: row.get(4)?,
                expires_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn delete_annotation(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM annotations WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear_expired_annotations(&self) -> Result<usize> {
        let now = chrono::Utc::now().to_rfc3339();
        let count = self.conn.execute(
            "DELETE FROM annotations WHERE expires_at IS NOT NULL AND expires_at <= ?1",
            params![now],
        )?;
        Ok(count)
    }

    pub fn clear_annotations(&self, session_id: Option<&str>) -> Result<usize> {
        let count = match session_id {
            Some(sid) => self.conn.execute(
                "DELETE FROM annotations WHERE session_id = ?1",
                params![sid],
            )?,
            None => self.conn.execute("DELETE FROM annotations", [])?,
        };
        Ok(count)
    }

    // ── Working Set ──

    pub fn add_to_workset(
        &self,
        path: &str,
        kind: &str,
        session_id: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO workset (path, kind, session_id, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![path, kind, session_id, now],
        )?;
        Ok(())
    }

    pub fn get_workset(
        &self,
        kind: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<Vec<WorksetEntry>> {
        let mut sql =
            String::from("SELECT id, path, kind, session_id, created_at FROM workset WHERE 1=1");
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(k) = kind {
            sql.push_str(&format!(" AND kind = ?{idx}"));
            params_vec.push(Box::new(k.to_string()));
            idx += 1;
        }
        if let Some(s) = session_id {
            sql.push_str(&format!(" AND (session_id IS NULL OR session_id = ?{idx})"));
            params_vec.push(Box::new(s.to_string()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(WorksetEntry {
                id: row.get(0)?,
                path: row.get(1)?,
                kind: row.get(2)?,
                session_id: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn remove_from_workset(&self, path: &str, kind: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM workset WHERE path = ?1 AND kind = ?2",
            params![path, kind],
        )?;
        Ok(())
    }

    pub fn clear_workset(&self, session_id: Option<&str>) -> Result<usize> {
        let count = if let Some(s) = session_id {
            self.conn
                .execute("DELETE FROM workset WHERE session_id = ?1", params![s])?
        } else {
            self.conn.execute("DELETE FROM workset", [])?
        };
        Ok(count)
    }

    /// Get focused paths for use in search ranking.
    pub fn get_focus_paths(&self, session_id: Option<&str>) -> Result<Vec<String>> {
        let entries = self.get_workset(Some("focus"), session_id)?;
        Ok(entries.into_iter().map(|e| e.path).collect())
    }

    /// Get visited paths for use in search ranking.
    pub fn get_visited_paths(&self, session_id: Option<&str>) -> Result<Vec<String>> {
        let entries = self.get_workset(Some("visited"), session_id)?;
        Ok(entries.into_iter().map(|e| e.path).collect())
    }

    // ── Embeddings ──

    pub fn upsert_embedding(&self, chunk_id: i64, model: &str, embedding: &[f32]) -> Result<()> {
        let blob = embedding_to_blob(embedding);
        self.conn.execute(
            "INSERT INTO embeddings (chunk_id, model, embedding)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(chunk_id) DO UPDATE SET model = ?2, embedding = ?3",
            params![chunk_id, model, blob],
        )?;
        Ok(())
    }

    pub fn upsert_embeddings_batch(
        &self,
        entries: &[(i64, &str, &[f32])],
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO embeddings (chunk_id, model, embedding)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(chunk_id) DO UPDATE SET model = ?2, embedding = ?3",
            )?;
            for (chunk_id, model, embedding) in entries {
                let blob = embedding_to_blob(embedding);
                stmt.execute(params![chunk_id, model, blob])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Get all chunk IDs that lack an embedding (or have one from a different model).
    pub fn chunks_needing_embedding(&self, model: &str) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.content FROM chunks c
             LEFT JOIN embeddings e ON c.id = e.chunk_id AND e.model = ?1
             WHERE e.chunk_id IS NULL",
        )?;
        let rows = stmt.query_map(params![model], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Load all embeddings for vector search. Returns (chunk_id, embedding).
    pub fn all_embeddings(&self) -> Result<Vec<(i64, Vec<f32>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT chunk_id, embedding FROM embeddings",
        )?;
        let rows = stmt.query_map([], |row| {
            let blob: Vec<u8> = row.get(1)?;
            Ok((row.get::<_, i64>(0)?, blob))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (id, blob) = row?;
            result.push((id, blob_to_embedding(&blob)));
        }
        Ok(result)
    }

    /// Load a chunk by ID (for building search results from vector matches).
    pub fn chunk_by_id(&self, chunk_id: i64) -> Result<Option<SearchResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.path, f.language, c.kind, c.name, c.signature, c.start_line, c.end_line, c.content
             FROM chunks c JOIN files f ON c.file_id = f.id
             WHERE c.id = ?1",
        )?;
        let mut rows = stmt.query_map(params![chunk_id], |row| {
            Ok(SearchResult {
                file_path: row.get(0)?,
                language: row.get(1)?,
                chunk_kind: row.get(2)?,
                chunk_name: row.get(3)?,
                signature: row.get(4)?,
                start_line: row.get(5)?,
                end_line: row.get(6)?,
                content: row.get(7)?,
                rank: 0.0,
            })
        })?;
        match rows.next() {
            Some(Ok(r)) => Ok(Some(r)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    pub fn chunk_count(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM chunks", [], |r| r.get(0),
        )?;
        Ok(count)
    }

    pub fn embedding_count(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM embeddings", [], |r| r.get(0),
        )?;
        Ok(count)
    }

    pub fn stats(&self, storage_dir: &Path) -> Result<IndexStats> {
        let file_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
        let chunk_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
        let total_size_bytes: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(size_bytes), 0) FROM files",
            [],
            |r| r.get(0),
        )?;

        let db_path = storage_dir.join("index.db");
        let db_size_bytes = std::fs::metadata(&db_path)
            .map(|m| m.len())
            .unwrap_or(0);

        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(language, 'unknown'), COUNT(*) FROM files GROUP BY language ORDER BY COUNT(*) DESC",
        )?;
        let languages: Vec<(String, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(IndexStats {
            file_count,
            chunk_count,
            total_size_bytes,
            db_size_bytes,
            languages,
        })
    }
}

fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(embedding.len() * 4);
    for &val in embedding {
        blob.extend_from_slice(&val.to_le_bytes());
    }
    blob
}

fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_store() -> (TempDir, Store) {
        let dir = TempDir::new().unwrap();
        let store = Store::open(dir.path()).unwrap();
        (dir, store)
    }

    fn insert_test_file(store: &Store, path: &str, lang: &str) -> i64 {
        let fid = store.upsert_file(path, "hash123", 100, Some(lang)).unwrap();
        store.insert_chunks(fid, &[
            ChunkInsert {
                kind: "function".into(),
                name: Some("hello".into()),
                content: "fn hello() { println!(\"hi\"); }".into(),
                signature: Some("fn hello()".into()),
                start_line: 1,
                end_line: 3,
                start_byte: 0,
                end_byte: 30,
            },
            ChunkInsert {
                kind: "function".into(),
                name: Some("world".into()),
                content: "fn world() -> i32 { 42 }".into(),
                signature: Some("fn world() -> i32".into()),
                start_line: 5,
                end_line: 7,
                start_byte: 31,
                end_byte: 55,
            },
            ChunkInsert {
                kind: "struct".into(),
                name: Some("Config".into()),
                content: "struct Config { name: String }".into(),
                signature: Some("struct Config".into()),
                start_line: 9,
                end_line: 11,
                start_byte: 56,
                end_byte: 85,
            },
        ]).unwrap();
        fid
    }

    #[test]
    fn open_creates_db() {
        let dir = TempDir::new().unwrap();
        assert!(!dir.path().join("index.db").exists());
        let _store = Store::open(dir.path()).unwrap();
        assert!(dir.path().join("index.db").exists());
    }

    #[test]
    fn open_if_exists_returns_none_for_missing() {
        let dir = TempDir::new().unwrap();
        let result = Store::open_if_exists(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn open_if_exists_returns_some_for_existing() {
        let (dir, _store) = test_store();
        let result = Store::open_if_exists(dir.path()).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn upsert_and_get_file() {
        let (_dir, store) = test_store();
        let fid = store.upsert_file("src/main.rs", "abc123", 500, Some("rust")).unwrap();
        assert!(fid > 0);

        let file = store.get_file("src/main.rs").unwrap();
        assert!(file.is_some());
        let file = file.unwrap();
        assert_eq!(file.path, "src/main.rs");
        assert_eq!(file.content_hash, "abc123");
        assert_eq!(file.size_bytes, 500);
        assert_eq!(file.language, Some("rust".into()));
    }

    #[test]
    fn upsert_file_updates_on_conflict() {
        let (_dir, store) = test_store();
        store.upsert_file("src/main.rs", "hash1", 100, Some("rust")).unwrap();
        store.upsert_file("src/main.rs", "hash2", 200, Some("rust")).unwrap();

        let file = store.get_file("src/main.rs").unwrap().unwrap();
        assert_eq!(file.content_hash, "hash2");
        assert_eq!(file.size_bytes, 200);
    }

    #[test]
    fn get_file_returns_none_for_missing() {
        let (_dir, store) = test_store();
        assert!(store.get_file("nonexistent.rs").unwrap().is_none());
    }

    #[test]
    fn insert_and_search_chunks() {
        let (_dir, store) = test_store();
        insert_test_file(&store, "src/lib.rs", "rust");

        let results = store.search("hello", None, None, None, 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].chunk_name, Some("hello".into()));
        assert_eq!(results[0].file_path, "src/lib.rs");
    }

    #[test]
    fn search_with_language_filter() {
        let (_dir, store) = test_store();
        insert_test_file(&store, "src/lib.rs", "rust");
        insert_test_file(&store, "src/app.py", "python");

        let rust_results = store.search("hello", Some("rust"), None, None, 10).unwrap();
        assert_eq!(rust_results.len(), 1);
        assert_eq!(rust_results[0].file_path, "src/lib.rs");

        let py_results = store.search("hello", Some("python"), None, None, 10).unwrap();
        assert_eq!(py_results.len(), 1);
        assert_eq!(py_results[0].file_path, "src/app.py");
    }

    #[test]
    fn search_with_kind_filter() {
        let (_dir, store) = test_store();
        insert_test_file(&store, "src/lib.rs", "rust");

        let fns = store.search("hello", None, None, Some("function"), 10).unwrap();
        assert!(!fns.is_empty());

        let structs = store.search("hello", None, None, Some("struct"), 10).unwrap();
        assert!(structs.is_empty());
    }

    #[test]
    fn search_with_path_prefix() {
        let (_dir, store) = test_store();
        insert_test_file(&store, "src/lib.rs", "rust");
        insert_test_file(&store, "tests/test.rs", "rust");

        let src = store.search("hello", None, Some("src/"), None, 10).unwrap();
        assert_eq!(src.len(), 1);
        assert_eq!(src[0].file_path, "src/lib.rs");
    }

    #[test]
    fn search_returns_signature() {
        let (_dir, store) = test_store();
        insert_test_file(&store, "src/lib.rs", "rust");

        let results = store.search("world", None, None, None, 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].signature, Some("fn world() -> i32".into()));
    }

    #[test]
    fn search_no_results() {
        let (_dir, store) = test_store();
        insert_test_file(&store, "src/lib.rs", "rust");

        let results = store.search("nonexistentxyz", None, None, None, 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn list_symbols_excludes_raw() {
        let (_dir, store) = test_store();
        let fid = store.upsert_file("src/lib.rs", "h", 100, Some("rust")).unwrap();
        store.insert_chunks(fid, &[
            ChunkInsert { kind: "function".into(), name: Some("a".into()), content: "fn a()".into(), signature: None, start_line: 1, end_line: 1, start_byte: 0, end_byte: 6 },
            ChunkInsert { kind: "raw".into(), name: None, content: "raw text".into(), signature: None, start_line: 2, end_line: 2, start_byte: 7, end_byte: 15 },
        ]).unwrap();

        let symbols = store.list_symbols(None, None).unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].chunk_kind, "function");
    }

    #[test]
    fn all_chunks_includes_raw() {
        let (_dir, store) = test_store();
        let fid = store.upsert_file("src/lib.rs", "h", 100, Some("rust")).unwrap();
        store.insert_chunks(fid, &[
            ChunkInsert { kind: "function".into(), name: Some("a".into()), content: "fn a()".into(), signature: None, start_line: 1, end_line: 1, start_byte: 0, end_byte: 6 },
            ChunkInsert { kind: "raw".into(), name: None, content: "raw text".into(), signature: None, start_line: 2, end_line: 2, start_byte: 7, end_byte: 15 },
        ]).unwrap();

        let all = store.all_chunks(None, None).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn delete_chunks_for_file() {
        let (_dir, store) = test_store();
        let fid = insert_test_file(&store, "src/lib.rs", "rust");

        let before = store.all_chunks(None, None).unwrap();
        assert_eq!(before.len(), 3);

        store.delete_chunks_for_file(fid).unwrap();
        let after = store.all_chunks(None, None).unwrap();
        assert!(after.is_empty());
    }

    #[test]
    fn remove_file_cascades() {
        let (_dir, store) = test_store();
        insert_test_file(&store, "src/lib.rs", "rust");

        store.remove_file("src/lib.rs").unwrap();
        assert!(store.get_file("src/lib.rs").unwrap().is_none());
        assert!(store.all_chunks(None, None).unwrap().is_empty());
    }

    #[test]
    fn stats() {
        let (dir, store) = test_store();
        insert_test_file(&store, "src/lib.rs", "rust");

        let stats = store.stats(dir.path()).unwrap();
        assert_eq!(stats.file_count, 1);
        assert_eq!(stats.chunk_count, 3);
        assert!(stats.total_size_bytes > 0);
        assert_eq!(stats.languages.len(), 1);
        assert_eq!(stats.languages[0].0, "rust");
    }

    #[test]
    fn kind_stats() {
        let (_dir, store) = test_store();
        insert_test_file(&store, "src/lib.rs", "rust");

        let kinds = store.kind_stats().unwrap();
        let fn_count = kinds.iter().find(|(k, _)| k == "function").map(|(_, c)| *c).unwrap_or(0);
        let st_count = kinds.iter().find(|(k, _)| k == "struct").map(|(_, c)| *c).unwrap_or(0);
        assert_eq!(fn_count, 2);
        assert_eq!(st_count, 1);
    }

    // ── Annotations ──

    #[test]
    fn annotation_crud() {
        let (_dir, store) = test_store();
        let id = store.add_annotation("src/lib.rs", "important file", None, None).unwrap();
        assert!(id > 0);

        let anns = store.get_annotations(None, None).unwrap();
        assert_eq!(anns.len(), 1);
        assert_eq!(anns[0].target, "src/lib.rs");
        assert_eq!(anns[0].note, "important file");

        store.delete_annotation(id).unwrap();
        let anns = store.get_annotations(None, None).unwrap();
        assert!(anns.is_empty());
    }

    #[test]
    fn annotation_session_filter() {
        let (_dir, store) = test_store();
        store.add_annotation("a.rs", "note1", Some("s1"), None).unwrap();
        store.add_annotation("b.rs", "note2", Some("s2"), None).unwrap();
        store.add_annotation("c.rs", "note3", None, None).unwrap();

        let s1 = store.get_annotations(None, Some("s1")).unwrap();
        assert_eq!(s1.len(), 2); // s1 + global (None)

        let all = store.get_annotations(None, None).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn annotation_target_filter() {
        let (_dir, store) = test_store();
        store.add_annotation("a.rs", "note1", None, None).unwrap();
        store.add_annotation("b.rs", "note2", None, None).unwrap();

        let filtered = store.get_annotations(Some("a.rs"), None).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].target, "a.rs");
    }

    #[test]
    fn clear_annotations_all() {
        let (_dir, store) = test_store();
        store.add_annotation("a.rs", "n1", Some("s1"), None).unwrap();
        store.add_annotation("b.rs", "n2", Some("s2"), None).unwrap();
        store.add_annotation("c.rs", "n3", None, None).unwrap();

        let cleared = store.clear_annotations(None).unwrap();
        assert_eq!(cleared, 3);
        assert!(store.get_annotations(None, None).unwrap().is_empty());
    }

    #[test]
    fn clear_annotations_by_session() {
        let (_dir, store) = test_store();
        store.add_annotation("a.rs", "n1", Some("s1"), None).unwrap();
        store.add_annotation("b.rs", "n2", Some("s2"), None).unwrap();

        let cleared = store.clear_annotations(Some("s1")).unwrap();
        assert_eq!(cleared, 1);
        let remaining = store.get_annotations(None, None).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].target, "b.rs");
    }

    // ── Working Set ──

    #[test]
    fn workset_focus_and_visit() {
        let (_dir, store) = test_store();
        store.add_to_workset("src/main.rs", "focus", None).unwrap();
        store.add_to_workset("src/old.rs", "visited", None).unwrap();

        let focus = store.get_focus_paths(None).unwrap();
        assert_eq!(focus, vec!["src/main.rs"]);

        let visited = store.get_visited_paths(None).unwrap();
        assert_eq!(visited, vec!["src/old.rs"]);
    }

    #[test]
    fn clear_workset() {
        let (_dir, store) = test_store();
        store.add_to_workset("a", "focus", Some("s1")).unwrap();
        store.add_to_workset("b", "focus", Some("s2")).unwrap();

        store.clear_workset(Some("s1")).unwrap();
        let focus = store.get_focus_paths(None).unwrap();
        assert_eq!(focus, vec!["b"]);

        store.clear_workset(None).unwrap();
        assert!(store.get_focus_paths(None).unwrap().is_empty());
    }

    // ── Changed Since ──

    #[test]
    fn chunks_changed_since() {
        let (_dir, store) = test_store();
        let old_ts = "2020-01-01T00:00:00Z";
        insert_test_file(&store, "src/lib.rs", "rust");

        let changed = store.chunks_changed_since(old_ts, None).unwrap();
        assert_eq!(changed.len(), 3); // all 3 chunks are newer than 2020

        let future_ts = "2099-01-01T00:00:00Z";
        let changed = store.chunks_changed_since(future_ts, None).unwrap();
        assert!(changed.is_empty());
    }

    #[test]
    fn chunks_changed_since_with_kind_filter() {
        let (_dir, store) = test_store();
        insert_test_file(&store, "src/lib.rs", "rust");

        let fns = store.chunks_changed_since("2020-01-01T00:00:00Z", Some("function")).unwrap();
        assert_eq!(fns.len(), 2);

        let structs = store.chunks_changed_since("2020-01-01T00:00:00Z", Some("struct")).unwrap();
        assert_eq!(structs.len(), 1);
    }

    // ── Embeddings ──

    #[test]
    fn embedding_roundtrip() {
        let (_dir, store) = test_store();
        insert_test_file(&store, "src/lib.rs", "rust");

        let actual_id: i64 = store.conn.query_row("SELECT id FROM chunks LIMIT 1", [], |r| r.get(0)).unwrap();

        let embedding = vec![0.1f32, 0.2, 0.3, 0.4];
        store.upsert_embedding(actual_id, "test-model", &embedding).unwrap();

        let count = store.embedding_count().unwrap();
        assert_eq!(count, 1);
    }

    // ── FTS Sanitization ──

    #[test]
    fn sanitize_plain_query() {
        assert_eq!(sanitize_fts_query("hello world"), "hello world");
    }

    #[test]
    fn sanitize_hyphenated_query() {
        assert_eq!(sanitize_fts_query("tree-sitter"), "\"tree-sitter\"");
    }

    #[test]
    fn sanitize_quoted_phrase() {
        assert_eq!(sanitize_fts_query("\"exact match\""), "\"exact match\"");
    }

    #[test]
    fn sanitize_mixed_query() {
        assert_eq!(sanitize_fts_query("hello tree-sitter world"), "hello \"tree-sitter\" world");
    }

    #[test]
    fn sanitize_path_query() {
        assert_eq!(sanitize_fts_query("src/main.rs"), "\"src/main.rs\"");
    }

    #[test]
    fn sanitize_empty() {
        assert_eq!(sanitize_fts_query(""), "");
    }

    // ── Transactions ──

    #[test]
    fn transaction_commit() {
        let (_dir, store) = test_store();
        store.begin_transaction().unwrap();
        store.upsert_file("a.rs", "h", 10, None).unwrap();
        store.commit_transaction().unwrap();
        assert!(store.get_file("a.rs").unwrap().is_some());
    }

    #[test]
    fn transaction_rollback() {
        let (_dir, store) = test_store();
        store.begin_transaction().unwrap();
        store.upsert_file("a.rs", "h", 10, None).unwrap();
        store.rollback_transaction().unwrap();
        assert!(store.get_file("a.rs").unwrap().is_none());
    }
}

/// Sanitize user input for FTS5 MATCH queries.
/// FTS5 treats `-`, `AND`, `OR`, `NOT`, `NEAR` as operators.
/// We quote bare terms that contain special characters, and preserve
/// user-supplied phrases (already in double quotes).
fn sanitize_fts_query(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch == '"' {
            // Pass through quoted phrases verbatim
            result.push(ch);
            chars.next();
            while let Some(&c) = chars.peek() {
                result.push(c);
                chars.next();
                if c == '"' {
                    break;
                }
            }
        } else if ch.is_whitespace() {
            result.push(ch);
            chars.next();
        } else {
            // Collect a bare token
            let mut token = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == '"' {
                    break;
                }
                token.push(c);
                chars.next();
            }
            let needs_quoting = token.contains('-')
                || token.contains('.')
                || token.contains('/')
                || token.contains(':')
                || token.contains('*')
                || token.contains('^');
            if needs_quoting {
                result.push('"');
                result.push_str(&token);
                result.push('"');
            } else {
                result.push_str(&token);
            }
        }
    }

    result
}
