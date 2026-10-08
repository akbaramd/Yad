use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{model::SearchDocument, project::Project};

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone)]
pub struct MemoryIndexRecord {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub space: String,
    pub subject: Option<String>,
    pub status: String,
    pub text_hash: String,
    pub exact_key: Option<String>,
    pub subject_key: Option<String>,
}

pub fn database_path(project: &Project) -> PathBuf {
    project.runtime_dir().join("yad.sqlite3")
}

fn open(project: &Project) -> Result<Connection> {
    fs::create_dir_all(project.runtime_dir())?;
    let path = database_path(project);
    let connection = Connection::open(&path)
        .with_context(|| format!("failed to open runtime database {}", path.display()))?;

    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .context("failed to configure SQLite busy timeout")?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .context("failed to enable SQLite WAL mode")?;
    connection
        .pragma_update(None, "synchronous", "NORMAL")
        .context("failed to configure SQLite synchronous mode")?;

    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS runtime_meta (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS memory_index (
            id TEXT PRIMARY KEY NOT NULL,
            path TEXT NOT NULL,
            kind TEXT NOT NULL,
            space TEXT NOT NULL,
            subject TEXT,
            status TEXT NOT NULL,
            text_hash TEXT NOT NULL,
            exact_key TEXT,
            subject_key TEXT
        );

        CREATE TABLE IF NOT EXISTS document_index (
            id TEXT PRIMARY KEY NOT NULL,
            source_type TEXT NOT NULL,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            space TEXT NOT NULL,
            status TEXT NOT NULL,
            path TEXT NOT NULL,
            text TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS ix_document_space ON document_index(space);
        CREATE INDEX IF NOT EXISTS ix_document_kind ON document_index(kind);
        CREATE INDEX IF NOT EXISTS ix_document_status ON document_index(status);

        CREATE UNIQUE INDEX IF NOT EXISTS ux_memory_exact_key
            ON memory_index(exact_key)
            WHERE exact_key IS NOT NULL;

        CREATE UNIQUE INDEX IF NOT EXISTS ux_memory_subject_key
            ON memory_index(subject_key)
            WHERE subject_key IS NOT NULL;
        "#,
    )?;

    connection.execute(
        "INSERT INTO runtime_meta(key, value) VALUES('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [SCHEMA_VERSION.to_string()],
    )?;

    Ok(connection)
}

pub fn catalog_ready(project: &Project) -> Result<bool> {
    let connection = open(project)?;
    let value = connection
        .query_row(
            "SELECT value FROM runtime_meta WHERE key='memory_catalog_ready'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("failed to read runtime catalog readiness")?;
    Ok(value.as_deref() == Some("1"))
}

pub fn catalog_git_head(project: &Project) -> Result<Option<String>> {
    let connection = open(project)?;
    connection
        .query_row(
            "SELECT value FROM runtime_meta WHERE key='memory_catalog_git_head'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("failed to read runtime catalog git head")
}

pub fn replace_memory_catalog(
    project: &Project,
    records: &[MemoryIndexRecord],
    git_head: Option<&str>,
) -> Result<()> {
    let mut connection = open(project)?;
    let transaction = connection.transaction()?;
    transaction.execute("DELETE FROM memory_index", [])?;

    {
        let mut statement = transaction.prepare(
            "INSERT INTO memory_index(
                id, path, kind, space, subject, status, text_hash, exact_key, subject_key
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;

        for record in records {
            statement.execute(params![
                record.id,
                record.path,
                record.kind,
                record.space,
                record.subject,
                record.status,
                record.text_hash,
                record.exact_key,
                record.subject_key,
            ])?;
        }
    }

    set_git_head_in_transaction(&transaction, git_head)?;
    transaction.execute(
        "INSERT INTO runtime_meta(key, value) VALUES('memory_catalog_ready', '1')
         ON CONFLICT(key) DO UPDATE SET value='1'",
        [],
    )?;
    transaction.commit()?;
    Ok(())
}

pub fn upsert_memory(
    project: &Project,
    record: &MemoryIndexRecord,
    git_head: Option<&str>,
) -> Result<()> {
    let mut connection = open(project)?;
    let transaction = connection.transaction()?;

    transaction.execute(
        r#"
        INSERT INTO memory_index(
            id, path, kind, space, subject, status, text_hash, exact_key, subject_key
        ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(id) DO UPDATE SET
            path=excluded.path,
            kind=excluded.kind,
            space=excluded.space,
            subject=excluded.subject,
            status=excluded.status,
            text_hash=excluded.text_hash,
            exact_key=excluded.exact_key,
            subject_key=excluded.subject_key
        "#,
        params![
            record.id,
            record.path,
            record.kind,
            record.space,
            record.subject,
            record.status,
            record.text_hash,
            record.exact_key,
            record.subject_key,
        ],
    )?;

    set_git_head_in_transaction(&transaction, git_head)?;
    transaction.commit()?;
    Ok(())
}

pub fn find_memory_path(project: &Project, id: &str) -> Result<Option<String>> {
    let connection = open(project)?;
    connection
        .query_row("SELECT path FROM memory_index WHERE id = ?1", [id], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .context("failed to query memory path")
}

pub fn find_exact_memory_id(project: &Project, exact_key: &str) -> Result<Option<String>> {
    let connection = open(project)?;
    connection
        .query_row(
            "SELECT id FROM memory_index WHERE exact_key = ?1",
            [exact_key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("failed to query exact memory key")
}

pub fn find_subject_memory_id(project: &Project, subject_key: &str) -> Result<Option<String>> {
    let connection = open(project)?;
    connection
        .query_row(
            "SELECT id FROM memory_index WHERE subject_key = ?1",
            [subject_key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("failed to query authoritative memory subject key")
}

pub fn memory_count(project: &Project) -> Result<usize> {
    let connection = open(project)?;
    let count: i64 =
        connection.query_row("SELECT COUNT(*) FROM memory_index", [], |row| row.get(0))?;
    Ok(count.max(0) as usize)
}

pub fn document_index_ready(project: &Project) -> Result<bool> {
    let connection = open(project)?;
    let value = connection
        .query_row(
            "SELECT value FROM runtime_meta WHERE key='document_index_ready'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("failed to read document index readiness")?;
    Ok(value.as_deref() == Some("1"))
}

pub fn document_index_git_head(project: &Project) -> Result<Option<String>> {
    let connection = open(project)?;
    connection
        .query_row(
            "SELECT value FROM runtime_meta WHERE key='document_index_git_head'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("failed to read document index git head")
}

pub fn replace_documents(
    project: &Project,
    documents: &[SearchDocument],
    git_head: Option<&str>,
) -> Result<()> {
    let mut connection = open(project)?;
    let transaction = connection.transaction()?;
    transaction.execute("DELETE FROM document_index", [])?;

    {
        let mut statement = transaction.prepare(
            "INSERT INTO document_index(
                id, source_type, kind, title, space, status, path, text
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;

        for document in documents {
            statement.execute(params![
                document.id,
                document.source_type,
                document.kind,
                document.title,
                document.space,
                document.status,
                document.path,
                document.text,
            ])?;
        }
    }

    set_document_git_head_in_transaction(&transaction, git_head)?;
    transaction.execute(
        "INSERT INTO runtime_meta(key, value) VALUES('document_index_ready', '1')
         ON CONFLICT(key) DO UPDATE SET value='1'",
        [],
    )?;
    transaction.commit()?;
    Ok(())
}

pub fn upsert_document(
    project: &Project,
    document: &SearchDocument,
    git_head: Option<&str>,
) -> Result<()> {
    let mut connection = open(project)?;
    let transaction = connection.transaction()?;
    transaction.execute(
        r#"
        INSERT INTO document_index(
            id, source_type, kind, title, space, status, path, text
        ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(id) DO UPDATE SET
            source_type=excluded.source_type,
            kind=excluded.kind,
            title=excluded.title,
            space=excluded.space,
            status=excluded.status,
            path=excluded.path,
            text=excluded.text
        "#,
        params![
            document.id,
            document.source_type,
            document.kind,
            document.title,
            document.space,
            document.status,
            document.path,
            document.text,
        ],
    )?;
    set_document_git_head_in_transaction(&transaction, git_head)?;
    transaction.commit()?;
    Ok(())
}

pub fn list_documents(project: &Project) -> Result<Vec<SearchDocument>> {
    let connection = open(project)?;
    let mut statement = connection.prepare(
        "SELECT id, source_type, kind, title, space, status, path, text
         FROM document_index",
    )?;

    let rows = statement.query_map([], |row| {
        Ok(SearchDocument {
            id: row.get(0)?,
            source_type: row.get(1)?,
            kind: row.get(2)?,
            title: row.get(3)?,
            space: row.get(4)?,
            status: row.get(5)?,
            path: row.get(6)?,
            text: row.get(7)?,
        })
    })?;

    let mut documents = Vec::new();
    for row in rows {
        documents.push(row?);
    }
    Ok(documents)
}

fn set_document_git_head_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    git_head: Option<&str>,
) -> Result<()> {
    match git_head {
        Some(value) => {
            transaction.execute(
                "INSERT INTO runtime_meta(key, value)
                 VALUES('document_index_git_head', ?1)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [value],
            )?;
        }
        None => {
            transaction.execute(
                "DELETE FROM runtime_meta WHERE key='document_index_git_head'",
                [],
            )?;
        }
    }
    Ok(())
}

fn set_git_head_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    git_head: Option<&str>,
) -> Result<()> {
    match git_head {
        Some(value) => {
            transaction.execute(
                "INSERT INTO runtime_meta(key, value)
                 VALUES('memory_catalog_git_head', ?1)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [value],
            )?;
        }
        None => {
            transaction.execute(
                "DELETE FROM runtime_meta WHERE key='memory_catalog_git_head'",
                [],
            )?;
        }
    }
    Ok(())
}
