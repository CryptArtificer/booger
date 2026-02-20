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
    ")?;

    conn.execute(
        "INSERT OR IGNORE INTO meta (key, value) VALUES ('schema_version', '2')",
        [],
    )?;

    Ok(())
}
