use anyhow::Result;
use rusqlite::Connection;

pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS files (
            id           INTEGER PRIMARY KEY,
            path         TEXT NOT NULL UNIQUE,
            content_hash TEXT NOT NULL,
            size_bytes   INTEGER NOT NULL,
            language     TEXT,
            indexed_at   TEXT NOT NULL,
            mtime        TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
        CREATE INDEX IF NOT EXISTS idx_files_language ON files(language);

        CREATE TABLE IF NOT EXISTS chunks (
            id         INTEGER PRIMARY KEY,
            file_id    INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            kind       TEXT NOT NULL,  -- 'function', 'struct', 'impl', 'class', 'module', 'block', 'raw'
            name       TEXT,           -- symbol name if applicable
            content    TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line   INTEGER NOT NULL,
            start_byte INTEGER NOT NULL,
            end_byte   INTEGER NOT NULL,
            UNIQUE(file_id, start_byte, end_byte)
        );
        CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_id);
        CREATE INDEX IF NOT EXISTS idx_chunks_kind ON chunks(kind);
        CREATE INDEX IF NOT EXISTS idx_chunks_name ON chunks(name) WHERE name IS NOT NULL;

        -- FTS5 virtual table for full-text search over chunk content and symbol names.
        -- content='' makes it an external-content table — we manage sync ourselves,
        -- which avoids doubling storage.
        CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
            name,
            content,
            content='chunks',
            content_rowid='id',
            tokenize='porter unicode61'
        );

        -- Triggers to keep FTS in sync with the chunks table.
        CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
            INSERT INTO chunks_fts(rowid, name, content) VALUES (new.id, new.name, new.content);
        END;
        CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
            INSERT INTO chunks_fts(chunks_fts, rowid, name, content) VALUES('delete', old.id, old.name, old.content);
        END;
        CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
            INSERT INTO chunks_fts(chunks_fts, rowid, name, content) VALUES('delete', old.id, old.name, old.content);
            INSERT INTO chunks_fts(rowid, name, content) VALUES (new.id, new.name, new.content);
        END;
        -- Volatile context: annotations attached to files/symbols/line-ranges.
        -- session_id scopes annotations to a session; NULL means persistent.
        -- expires_at enables TTL; NULL means no expiry.
        CREATE TABLE IF NOT EXISTS annotations (
            id         INTEGER PRIMARY KEY,
            target     TEXT NOT NULL,     -- file path, 'file:line', or symbol name
            note       TEXT NOT NULL,
            session_id TEXT,
            created_at TEXT NOT NULL,
            expires_at TEXT              -- ISO8601 timestamp, NULL = no expiry
        );
        CREATE INDEX IF NOT EXISTS idx_annotations_target ON annotations(target);
        CREATE INDEX IF NOT EXISTS idx_annotations_session ON annotations(session_id);

        -- Volatile context: working set — focused and visited/blacklisted paths.
        -- kind: 'focus' (boost in search) or 'visited' (deprioritize in search)
        CREATE TABLE IF NOT EXISTS workset (
            id         INTEGER PRIMARY KEY,
            path       TEXT NOT NULL,
            kind       TEXT NOT NULL CHECK(kind IN ('focus', 'visited')),
            session_id TEXT,
            created_at TEXT NOT NULL,
            UNIQUE(path, kind, session_id)
        );
        CREATE INDEX IF NOT EXISTS idx_workset_kind ON workset(kind);
        CREATE INDEX IF NOT EXISTS idx_workset_session ON workset(session_id);
    ")?;

    conn.execute(
        "INSERT OR IGNORE INTO meta (key, value) VALUES ('schema_version', '3')",
        [],
    )?;

    Ok(())
}
