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

impl Store {
    pub fn open(storage_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(storage_dir)
            .with_context(|| format!("creating storage dir {}", storage_dir.display()))?;
        let db_path = storage_dir.join("index.db");
        let conn = Connection::open(&db_path)
            .with_context(|| format!("opening database at {}", db_path.display()))?;
        schema::run_migrations(&conn)?;
        Ok(Self { conn })
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
            "INSERT OR REPLACE INTO chunks (file_id, kind, name, content, start_line, end_line, start_byte, end_byte)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for chunk in chunks {
            stmt.execute(params![
                file_id,
                chunk.kind,
                chunk.name,
                chunk.content,
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
        max_results: usize,
    ) -> Result<Vec<SearchResult>> {
        // Build the WHERE clause dynamically based on filters
        let mut sql = String::from(
            "SELECT f.path, f.language, c.kind, c.name, c.start_line, c.end_line, c.content,
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
        }

        sql.push_str(" ORDER BY chunks_fts.rank LIMIT ?");
        sql.push_str(&(param_idx + if path_prefix.is_some() { 1 } else { 0 }).to_string());

        let mut stmt = self.conn.prepare(&sql)?;

        // Bind parameters dynamically
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        params_vec.push(Box::new(query.to_string()));
        if let Some(lang) = language {
            params_vec.push(Box::new(lang.to_string()));
        }
        if let Some(prefix) = path_prefix {
            params_vec.push(Box::new(prefix.to_string()));
        }
        params_vec.push(Box::new(max_results as i64));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(SearchResult {
                file_path: row.get(0)?,
                language: row.get(1)?,
                chunk_kind: row.get(2)?,
                chunk_name: row.get(3)?,
                start_line: row.get(4)?,
                end_line: row.get(5)?,
                content: row.get(6)?,
                rank: row.get(7)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
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

    pub fn clear_session_annotations(&self, session_id: &str) -> Result<usize> {
        let count = self.conn.execute(
            "DELETE FROM annotations WHERE session_id = ?1",
            params![session_id],
        )?;
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

    /// Collect index statistics.
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
