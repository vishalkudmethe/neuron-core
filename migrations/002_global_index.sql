-- Neuron Global Index Schema — v5
-- File: ~/.neuron/global_index.sqlite
-- Applied by: project_manager::open_global_db()

-- All known projects across all machines and directories
CREATE TABLE IF NOT EXISTS projects (
    id              TEXT    NOT NULL PRIMARY KEY,        -- UUID v4, stable across machines
    name            TEXT    NOT NULL,                   -- Human-readable project name
    root_path       TEXT    NOT NULL UNIQUE,            -- Canonical root path (this machine)
    neuron_path     TEXT    NOT NULL,                   -- Path to .neuron/ folder
    language        TEXT    NOT NULL DEFAULT 'unknown', -- Primary language
    last_accessed   TEXT    NOT NULL,                   -- ISO-8601 UTC timestamp
    created_at      TEXT    NOT NULL,                   -- ISO-8601 UTC timestamp
    tags            TEXT    NOT NULL DEFAULT '[]'       -- JSON array of string tags
);

CREATE INDEX IF NOT EXISTS idx_proj_name     ON projects(name);
CREATE INDEX IF NOT EXISTS idx_proj_accessed ON projects(last_accessed DESC);
CREATE INDEX IF NOT EXISTS idx_proj_lang     ON projects(language);

-- Per-machine path aliases for portability
-- When a project is copied to a new machine, its root_path changes.
-- We record the alias here instead of rewriting the canonical project record.
CREATE TABLE IF NOT EXISTS path_aliases (
    project_id  TEXT    NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    machine_id  TEXT    NOT NULL,   -- First 16 hex chars of SHA-256(hostname:username)
    local_path  TEXT    NOT NULL,   -- Absolute path on this machine
    registered  TEXT    NOT NULL,   -- When this alias was first recorded
    PRIMARY KEY (project_id, machine_id)
);

CREATE INDEX IF NOT EXISTS idx_alias_machine ON path_aliases(machine_id);
CREATE INDEX IF NOT EXISTS idx_alias_project ON path_aliases(project_id);

-- Optional: project-level tags for grouping / filtering
CREATE TABLE IF NOT EXISTS project_tags (
    project_id  TEXT    NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    tag         TEXT    NOT NULL,
    PRIMARY KEY (project_id, tag)
);

-- Track last-known active project per machine (for auto-restore on startup)
CREATE TABLE IF NOT EXISTS machine_state (
    machine_id          TEXT    NOT NULL PRIMARY KEY,
    last_project_id     TEXT    REFERENCES projects(id),
    last_project_root   TEXT,
    updated_at          TEXT    NOT NULL
);
