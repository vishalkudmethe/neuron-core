-- Neuron Local Index Schema — v5
-- Migration: 001_initial
-- Applied by: search::bootstrap_local_db()

CREATE TABLE IF NOT EXISTS memory_units (
    id          TEXT    NOT NULL PRIMARY KEY,
    project_id  TEXT    NOT NULL DEFAULT '',
    unit_type   TEXT    NOT NULL,               -- 'file' | 'function' | 'struct' | 'enum' | 'trait' | 'class' | 'git_commit' | 'conversation'
    path        TEXT,                           -- Absolute file path
    symbol_name TEXT,                           -- Extracted symbol name
    language    TEXT,                           -- Detected language
    content     TEXT,                           -- Raw/snippet content (max 8KB)
    sha256      TEXT,                           -- SHA-256 of content for dedup
    embedding   BLOB,                           -- Reserved: vector embedding (v6)
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_mu_path      ON memory_units(path);
CREATE INDEX IF NOT EXISTS idx_mu_type      ON memory_units(unit_type);
CREATE INDEX IF NOT EXISTS idx_mu_lang      ON memory_units(language);
CREATE INDEX IF NOT EXISTS idx_mu_updated   ON memory_units(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_mu_project   ON memory_units(project_id);

-- FTS5 virtual table — full-text search across content, symbols, and paths
CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
    id          UNINDEXED,
    content,
    symbol_name,
    path,
    content     = 'memory_units',
    content_rowid = 'rowid'
);

-- Triggers to keep FTS in sync
CREATE TRIGGER IF NOT EXISTS memory_units_ai
    AFTER INSERT ON memory_units BEGIN
    INSERT INTO memory_fts(rowid, id, content, symbol_name, path)
    VALUES (new.rowid, new.id, new.content, new.symbol_name, new.path);
END;

CREATE TRIGGER IF NOT EXISTS memory_units_ad
    AFTER DELETE ON memory_units BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, id, content, symbol_name, path)
    VALUES ('delete', old.rowid, old.id, old.content, old.symbol_name, old.path);
END;

CREATE TRIGGER IF NOT EXISTS memory_units_au
    AFTER UPDATE ON memory_units BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, id, content, symbol_name, path)
    VALUES ('delete', old.rowid, old.id, old.content, old.symbol_name, old.path);
    INSERT INTO memory_fts(rowid, id, content, symbol_name, path)
    VALUES (new.rowid, new.id, new.content, new.symbol_name, new.path);
END;

-- Cross-project references
CREATE TABLE IF NOT EXISTS cross_refs (
    id              TEXT    NOT NULL PRIMARY KEY,
    source_project  TEXT    NOT NULL,
    target_project  TEXT    NOT NULL,
    source_unit     TEXT    NOT NULL REFERENCES memory_units(id),
    target_unit     TEXT    NOT NULL,
    ref_type        TEXT    NOT NULL,           -- 'depends_on' | 'copied_from' | 'mentioned_in'
    created_at      TEXT    NOT NULL
);

-- Loop guardian audit log
CREATE TABLE IF NOT EXISTS loop_events (
    id          TEXT    NOT NULL PRIMARY KEY,
    project_id  TEXT    NOT NULL DEFAULT '',
    pattern     TEXT    NOT NULL,
    count       INTEGER NOT NULL DEFAULT 1,
    first_seen  TEXT    NOT NULL,
    last_seen   TEXT    NOT NULL,
    terminated  INTEGER NOT NULL DEFAULT 0
);
