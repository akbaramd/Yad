use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, SecondsFormat, Utc};
use sha2::{Digest, Sha256};
use slug::slugify;
use ulid::Ulid;
use walkdir::WalkDir;

use crate::{
    atomic, frontmatter,
    model::{AdrMeta, MemoryMeta, RecordMeta, SearchDocument},
    project::{Project, validate_space},
    schema::SchemaDefinition,
    runtime_index::{self, MemoryIndexRecord},
};

#[derive(Debug, Clone)]
pub struct StoredMemory {
    pub meta: MemoryMeta,
    pub body: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StoredAdr {
    pub meta: AdrMeta,
    pub body: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct MemoryCreate {
    pub kind: String,
    pub text: String,
    pub space: String,
    pub subject: Option<String>,
    pub importance: u8,
    pub source: Option<String>,
    pub tags: Vec<String>,
    pub supersedes: Option<String>,
}

pub fn create_memory(project: &Project, input: MemoryCreate) -> Result<StoredMemory> {
    validate_memory_kind(&input.kind)?;
    validate_space(&input.space)?;
    project.ensure_space(&input.space)?;

    if input.text.trim().is_empty() {
        bail!("memory text cannot be empty");
    }
    if !(1..=5).contains(&input.importance) {
        bail!("importance must be between 1 and 5");
    }

    let now = Utc::now();
    let id = format!("MEM-{}", Ulid::new());
    let body = input.text.trim().to_string();
    let meta = MemoryMeta {
        yad: 1,
        schema: "memory".to_string(),
        schema_version: 1,
        id: id.clone(),
        kind: input.kind,
        space: normalize_space(&input.space),
        subject: input.subject,
        status: "active".to_string(),
        importance: input.importance,
        source: input.source,
        created: now.to_rfc3339_opts(SecondsFormat::Secs, true),
        updated: now.to_rfc3339_opts(SecondsFormat::Secs, true),
        tags: input.tags,
        supersedes: input.supersedes,
        superseded_by: None,
    };

    let dir = project
        .yad_dir
        .join("memory")
        .join(format!("{:04}", now.year()))
        .join(format!("{:02}", now.month()));
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{id}.md"));
    let raw = frontmatter::render(&meta, &body)?;
    atomic::write(&path, raw)?;

    let memory = StoredMemory { meta, body, path };
    refresh_memory_catalog_entry(project, &memory)?;
    refresh_document_index_if_ready(project, &memory_to_search_document(project, &memory))?;
    Ok(memory)
}

pub fn find_exact_active_memory(
    project: &Project,
    input: &MemoryCreate,
) -> Result<Option<StoredMemory>> {
    ensure_memory_catalog_current(project)?;
    let key = exact_memory_key(
        &input.kind,
        &normalize_space(&input.space),
        input.subject.as_deref(),
        &input.text,
    );

    let Some(id) = runtime_index::find_exact_memory_id(project, &key)? else {
        return Ok(None);
    };

    load_runtime_memory(project, &id).map(Some)
}

pub fn find_active_subject_conflict(
    project: &Project,
    input: &MemoryCreate,
) -> Result<Option<StoredMemory>> {
    if !requires_single_active_subject(&input.kind) {
        return Ok(None);
    }

    let Some(subject) = input.subject.as_deref().map(str::trim) else {
        return Ok(None);
    };
    if subject.is_empty() {
        return Ok(None);
    }

    ensure_memory_catalog_current(project)?;
    let key = authoritative_subject_key(&input.kind, &normalize_space(&input.space), subject);

    let Some(id) = runtime_index::find_subject_memory_id(project, &key)? else {
        return Ok(None);
    };

    let memory = load_runtime_memory(project, &id)?;
    if canonical_memory_text(&memory.body) == canonical_memory_text(&input.text) {
        Ok(None)
    } else {
        Ok(Some(memory))
    }
}

pub fn rebuild_memory_catalog(project: &Project) -> Result<usize> {
    let memories = list_memories(project)?;
    let mut exact_seen: HashMap<String, String> = HashMap::new();
    let mut subject_seen: HashMap<String, String> = HashMap::new();
    let mut records = Vec::with_capacity(memories.len());

    for memory in &memories {
        let record = memory_index_record(project, memory);

        if let Some(key) = &record.exact_key
            && let Some(existing) = exact_seen.insert(key.clone(), record.id.clone())
        {
            bail!(
                "duplicate active memory truth detected while rebuilding runtime index: {} and {}",
                existing,
                record.id
            );
        }

        if let Some(key) = &record.subject_key
            && let Some(existing) = subject_seen.insert(key.clone(), record.id.clone())
        {
            bail!(
                "conflicting active authoritative memories detected while rebuilding runtime index: {} and {}. Supersede one before sync.",
                existing,
                record.id
            );
        }

        records.push(record);
    }

    let head = current_git_head(project);
    runtime_index::replace_memory_catalog(project, &records, head.as_deref())?;
    Ok(records.len())
}

fn ensure_memory_catalog_current(project: &Project) -> Result<()> {
    let current_head = current_git_head(project);
    let ready = runtime_index::catalog_ready(project)?;
    let indexed_head = runtime_index::catalog_git_head(project)?;

    if !ready || indexed_head != current_head {
        rebuild_memory_catalog(project)?;
    }

    Ok(())
}

fn refresh_memory_catalog_entry(project: &Project, memory: &StoredMemory) -> Result<()> {
    ensure_memory_catalog_current(project)?;
    let record = memory_index_record(project, memory);
    let head = current_git_head(project);
    runtime_index::upsert_memory(project, &record, head.as_deref())
}

fn memory_index_record(project: &Project, memory: &StoredMemory) -> MemoryIndexRecord {
    let active = memory.meta.status == "active";
    let exact_key = active.then(|| {
        exact_memory_key(
            &memory.meta.kind,
            &memory.meta.space,
            memory.meta.subject.as_deref(),
            &memory.body,
        )
    });

    let subject_key = if active && requires_single_active_subject(&memory.meta.kind) {
        memory
            .meta
            .subject
            .as_deref()
            .map(str::trim)
            .filter(|subject| !subject.is_empty())
            .map(|subject| {
                authoritative_subject_key(&memory.meta.kind, &memory.meta.space, subject)
            })
    } else {
        None
    };

    MemoryIndexRecord {
        id: memory.meta.id.clone(),
        path: relative_path(project, &memory.path),
        kind: memory.meta.kind.clone(),
        space: memory.meta.space.clone(),
        subject: memory.meta.subject.clone(),
        status: memory.meta.status.clone(),
        text_hash: hash_text(&canonical_memory_text(&memory.body)),
        exact_key,
        subject_key,
    }
}

fn load_runtime_memory(project: &Project, id: &str) -> Result<StoredMemory> {
    let path = runtime_index::find_memory_path(project, id)?
        .with_context(|| format!("memory '{}' not present in runtime index", id))?;
    parse_memory_file(&project.root.join(path))
}

fn current_git_head(project: &Project) -> Option<String> {
    let dot_git = project.root.join(".git");
    let git_dir = if dot_git.is_dir() {
        dot_git
    } else {
        let raw = fs::read_to_string(&dot_git).ok()?;
        let path = raw.trim().strip_prefix("gitdir:")?.trim();
        let candidate = PathBuf::from(path);
        if candidate.is_absolute() {
            candidate
        } else {
            project.root.join(candidate)
        }
    };

    let common_dir = git_common_dir(&git_dir);
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();

    if let Some(reference) = head.strip_prefix("ref:") {
        let reference = reference.trim();
        let loose = common_dir.join(reference);
        if let Ok(value) = fs::read_to_string(&loose) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }

        let packed = fs::read_to_string(common_dir.join("packed-refs")).ok()?;
        for line in packed.lines() {
            if line.is_empty() || line.starts_with('#') || line.starts_with('^') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let oid = parts.next()?;
            let name = parts.next()?;
            if name == reference {
                return Some(oid.to_string());
            }
        }
        None
    } else if head.is_empty() {
        None
    } else {
        Some(head.to_string())
    }
}

fn git_common_dir(git_dir: &Path) -> PathBuf {
    let common = git_dir.join("commondir");
    if let Ok(raw) = fs::read_to_string(common) {
        let path = PathBuf::from(raw.trim());
        if path.is_absolute() {
            return path;
        }
        return git_dir.join(path);
    }
    git_dir.to_path_buf()
}

fn exact_memory_key(kind: &str, space: &str, subject: Option<&str>, text: &str) -> String {
    let subject = subject.unwrap_or_default().trim().to_lowercase();
    let raw = format!(
        "{}\0{}\0{}\0{}",
        kind.to_ascii_lowercase(),
        space,
        subject,
        canonical_memory_text(text)
    );
    hash_text(&raw)
}

fn authoritative_subject_key(kind: &str, space: &str, subject: &str) -> String {
    let raw = format!(
        "{}\0{}\0{}",
        kind.to_ascii_lowercase(),
        space,
        subject.trim().to_lowercase()
    );
    hash_text(&raw)
}

fn hash_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn requires_single_active_subject(kind: &str) -> bool {
    matches!(
        kind.to_ascii_lowercase().as_str(),
        "fact" | "decision" | "state"
    )
}

pub fn find_memory(project: &Project, id: &str) -> Result<StoredMemory> {
    ensure_memory_catalog_current(project)?;

    if let Some(path) = runtime_index::find_memory_path(project, id)? {
        return parse_memory_file(&project.root.join(path));
    }

    for entry in WalkDir::new(project.yad_dir.join("memory")) {
        let entry = entry.context("failed while traversing .yad/memory")?;
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("md")
        {
            continue;
        }

        let memory = parse_memory_file(entry.path())?;
        if memory.meta.id.eq_ignore_ascii_case(id) {
            rebuild_memory_catalog(project)?;
            return Ok(memory);
        }
    }

    bail!("memory '{}' not found", id)
}

pub fn save_memory(project: &Project, memory: &StoredMemory) -> Result<()> {
    atomic::write(
        &memory.path,
        frontmatter::render(&memory.meta, &memory.body)?,
    )?;
    refresh_memory_catalog_entry(project, memory)?;
    refresh_document_index_if_ready(project, &memory_to_search_document(project, memory))
}

pub fn list_memories(project: &Project) -> Result<Vec<StoredMemory>> {
    let mut result = Vec::new();
    let root = project.yad_dir.join("memory");
    if !root.exists() {
        return Ok(result);
    }

    for entry in WalkDir::new(root) {
        let entry = entry.context("failed while traversing .yad/memory")?;
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("md")
        {
            continue;
        }

        result.push(parse_memory_file(entry.path())?);
    }

    result.sort_by(|a, b| b.meta.created.cmp(&a.meta.created));
    Ok(result)
}

fn parse_memory_file(path: &Path) -> Result<StoredMemory> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read memory {}", path.display()))?;
    let (meta, body) = frontmatter::parse::<MemoryMeta>(&raw)
        .with_context(|| format!("invalid memory document {}", path.display()))?;

    validate_memory_document(&meta, &body, path)?;

    Ok(StoredMemory {
        meta,
        body,
        path: path.to_path_buf(),
    })
}

fn validate_memory_document(meta: &MemoryMeta, body: &str, path: &Path) -> Result<()> {
    if meta.yad != 1 {
        bail!(
            "{} has unsupported yad version {}",
            path.display(),
            meta.yad
        );
    }
    if meta.schema != "memory" {
        bail!(
            "{} declares schema '{}' instead of 'memory'",
            path.display(),
            meta.schema
        );
    }
    if meta.schema_version != 1 {
        bail!(
            "{} has unsupported memory schema version {}",
            path.display(),
            meta.schema_version
        );
    }
    if !meta.id.starts_with("MEM-") {
        bail!("{} has invalid memory id '{}'", path.display(), meta.id);
    }

    validate_memory_kind(&meta.kind)
        .with_context(|| format!("{} has invalid memory kind", path.display()))?;
    validate_space(&meta.space)
        .with_context(|| format!("{} has invalid memory space", path.display()))?;

    if !matches!(meta.status.as_str(), "active" | "archived" | "superseded") {
        bail!(
            "{} has invalid memory status '{}'",
            path.display(),
            meta.status
        );
    }
    if !(1..=5).contains(&meta.importance) {
        bail!(
            "{} has invalid memory importance {}",
            path.display(),
            meta.importance
        );
    }
    if body.trim().is_empty() {
        bail!("{} has an empty memory body", path.display());
    }

    DateTime::parse_from_rfc3339(&meta.created)
        .with_context(|| format!("{} has invalid created timestamp", path.display()))?;
    DateTime::parse_from_rfc3339(&meta.updated)
        .with_context(|| format!("{} has invalid updated timestamp", path.display()))?;

    if meta.status == "superseded" && meta.superseded_by.is_none() {
        bail!(
            "{} is superseded but does not reference superseded_by",
            path.display()
        );
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct StoredRecord {
    pub meta: RecordMeta,
    pub body: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RecordCreate {
    pub title: String,
    pub space: String,
    pub sections: BTreeMap<String, String>,
    pub tags: Vec<String>,
}

pub fn create_record(
    project: &Project,
    definition: &SchemaDefinition,
    input: RecordCreate,
) -> Result<StoredRecord> {
    validate_space(&input.space)?;
    project.ensure_space(&input.space)?;

    if input.title.trim().is_empty() {
        bail!("record title cannot be empty");
    }

    for key in input.sections.keys() {
        if definition.section(key).is_none() {
            bail!(
                "schema '{}' does not define section '{}'",
                definition.id,
                key
            );
        }
    }

    let now = Utc::now();
    let ulid = Ulid::new().to_string();
    let suffix = &ulid[ulid.len() - 10..];
    let id = format!(
        "{}-{}-{}",
        definition.id_prefix,
        now.format("%Y%m%d"),
        suffix
    );

    let meta = RecordMeta {
        yad: 1,
        schema: definition.id.clone(),
        schema_version: definition.version,
        id: id.clone(),
        title: input.title.trim().to_string(),
        status: definition.initial_status.clone(),
        space: normalize_space(&input.space),
        created: now.to_rfc3339_opts(SecondsFormat::Secs, true),
        updated: now.to_rfc3339_opts(SecondsFormat::Secs, true),
        tags: input.tags,
        supersedes: Vec::new(),
        superseded_by: Vec::new(),
    };

    let mut body = format!("# {}", meta.title);
    for section in &definition.sections {
        body.push_str(&format!("\n\n## {}\n\n", section.title));
        if let Some(value) = input.sections.get(&section.key) {
            body.push_str(value.trim());
        } else if section.required {
            body.push_str(&format!("<!-- required: {} -->", section.key));
        }
    }

    let dir = project
        .ensure_space(&input.space)?
        .join(&definition.directory);
    fs::create_dir_all(&dir)?;
    let slug = slugify(&meta.title);
    let filename = if slug.is_empty() {
        format!("{}.md", id)
    } else {
        format!("{}-{}.md", id, slug)
    };
    let path = dir.join(filename);

    atomic::write(&path, frontmatter::render(&meta, &body)?)?;

    let record = StoredRecord { meta, body, path };
    refresh_document_index_if_ready(project, &record_to_search_document(project, &record))?;
    Ok(record)
}

pub fn find_record(project: &Project, id: &str) -> Result<StoredRecord> {
    let root = project.yad_dir.join("spaces");
    if !root.exists() {
        bail!("record '{}' not found", id);
    }

    for entry in WalkDir::new(root) {
        let entry = entry.context("failed while traversing .yad/spaces")?;
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("md")
        {
            continue;
        }

        let record = parse_record_file(entry.path())?;
        if record.meta.id.eq_ignore_ascii_case(id) {
            return Ok(record);
        }
    }

    bail!("record '{}' not found", id)
}

pub fn save_record(project: &Project, record: &StoredRecord) -> Result<()> {
    atomic::write(
        &record.path,
        frontmatter::render(&record.meta, &record.body)?,
    )?;
    refresh_document_index_if_ready(project, &record_to_search_document(project, record))
}

pub fn list_records(project: &Project) -> Result<Vec<StoredRecord>> {
    let mut result = Vec::new();
    let root = project.yad_dir.join("spaces");
    if !root.exists() {
        return Ok(result);
    }

    for entry in WalkDir::new(root) {
        let entry = entry.context("failed while traversing .yad/spaces")?;
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("md")
        {
            continue;
        }

        result.push(parse_record_file(entry.path())?);
    }

    result.sort_by(|a, b| b.meta.created.cmp(&a.meta.created));
    Ok(result)
}

fn parse_record_file(path: &Path) -> Result<StoredRecord> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read record {}", path.display()))?;
    let (meta, body) = frontmatter::parse::<RecordMeta>(&raw)
        .with_context(|| format!("invalid record document {}", path.display()))?;

    if meta.yad != 1 {
        bail!(
            "{} has unsupported yad document version {}",
            path.display(),
            meta.yad
        );
    }
    if meta.schema.trim().is_empty() {
        bail!("{} has an empty record schema", path.display());
    }
    if meta.id.trim().is_empty() {
        bail!("{} has an empty record id", path.display());
    }
    if meta.title.trim().is_empty() {
        bail!("{} has an empty record title", path.display());
    }
    validate_space(&meta.space)
        .with_context(|| format!("{} has invalid record space", path.display()))?;
    DateTime::parse_from_rfc3339(&meta.created)
        .with_context(|| format!("{} has invalid created timestamp", path.display()))?;
    DateTime::parse_from_rfc3339(&meta.updated)
        .with_context(|| format!("{} has invalid updated timestamp", path.display()))?;

    Ok(StoredRecord {
        meta,
        body,
        path: path.to_path_buf(),
    })
}

pub fn create_adr(
    project: &Project,
    title: &str,
    space: &str,
    context: Option<&str>,
    decision: Option<&str>,
    consequences: Option<&str>,
    tags: Vec<String>,
) -> Result<StoredAdr> {
    validate_space(space)?;
    if title.trim().is_empty() {
        bail!("ADR title cannot be empty");
    }

    let now = Utc::now();
    let ulid = Ulid::new().to_string();
    let suffix = &ulid[ulid.len() - 10..];
    let id = format!("ADR-{}-{}", now.format("%Y%m%d"), suffix);
    let meta = AdrMeta {
        yad: 1,
        schema: "adr".to_string(),
        schema_version: 1,
        id: id.clone(),
        title: title.trim().to_string(),
        status: "proposed".to_string(),
        space: normalize_space(space),
        created: now.to_rfc3339_opts(SecondsFormat::Secs, true),
        updated: now.to_rfc3339_opts(SecondsFormat::Secs, true),
        tags,
        supersedes: Vec::new(),
        superseded_by: Vec::new(),
    };

    let placeholder = "<!-- required -->";
    let body = format!(
        "# {}\n\n## Context\n\n{}\n\n## Decision\n\n{}\n\n## Consequences\n\n{}",
        meta.title,
        context.unwrap_or(placeholder).trim(),
        decision.unwrap_or(placeholder).trim(),
        consequences.unwrap_or(placeholder).trim(),
    );

    let dir = project.ensure_space(space)?.join("adr");
    fs::create_dir_all(&dir)?;
    let slug = slugify(&meta.title);
    let filename = if slug.is_empty() {
        format!("{}.md", id)
    } else {
        format!("{}-{}.md", id, slug)
    };
    let path = dir.join(filename);
    atomic::write(&path, frontmatter::render(&meta, &body)?)?;

    let adr = StoredAdr { meta, body, path };
    refresh_document_index_if_ready(project, &adr_to_search_document(project, &adr))?;
    Ok(adr)
}

pub fn find_adr(project: &Project, id: &str) -> Result<StoredAdr> {
    let root = project.yad_dir.join("spaces");
    if !root.exists() {
        bail!("ADR '{}' not found", id);
    }

    for entry in WalkDir::new(root) {
        let entry = entry.context("failed while traversing .yad/spaces")?;
        if !is_adr_file(entry.path(), entry.file_type().is_file()) {
            continue;
        }

        let adr = parse_adr_file(entry.path())?;
        if adr.meta.id.eq_ignore_ascii_case(id) {
            return Ok(adr);
        }
    }

    bail!("ADR '{}' not found", id)
}

pub fn save_adr(project: &Project, adr: &StoredAdr) -> Result<()> {
    atomic::write(&adr.path, frontmatter::render(&adr.meta, &adr.body)?)?;
    refresh_document_index_if_ready(project, &adr_to_search_document(project, adr))
}

pub fn list_adrs(project: &Project) -> Result<Vec<StoredAdr>> {
    let mut result = Vec::new();
    let root = project.yad_dir.join("spaces");
    if !root.exists() {
        return Ok(result);
    }

    for entry in WalkDir::new(root) {
        let entry = entry.context("failed while traversing .yad/spaces")?;
        if !is_adr_file(entry.path(), entry.file_type().is_file()) {
            continue;
        }

        result.push(parse_adr_file(entry.path())?);
    }

    result.sort_by(|a, b| b.meta.created.cmp(&a.meta.created));
    Ok(result)
}

fn is_adr_file(path: &Path, is_file: bool) -> bool {
    is_file
        && path.extension().and_then(|value| value.to_str()) == Some("md")
        && path
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            == Some("adr")
}

fn parse_adr_file(path: &Path) -> Result<StoredAdr> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read ADR {}", path.display()))?;
    let (meta, body) = frontmatter::parse::<AdrMeta>(&raw)
        .with_context(|| format!("invalid ADR document {}", path.display()))?;

    if meta.schema != "adr" {
        bail!(
            "{} declares schema '{}' instead of 'adr'",
            path.display(),
            meta.schema
        );
    }

    Ok(StoredAdr {
        meta,
        body,
        path: path.to_path_buf(),
    })
}

pub fn collect_search_documents(project: &Project) -> Result<Vec<SearchDocument>> {
    let current_head = current_git_head(project);
    if runtime_index::document_index_ready(project)?
        && runtime_index::document_index_git_head(project)? == current_head
    {
        return runtime_index::list_documents(project);
    }

    collect_search_documents_from_source(project)
}

pub fn rebuild_document_index(project: &Project) -> Result<usize> {
    let documents = collect_search_documents_from_source(project)?;
    let head = current_git_head(project);
    runtime_index::replace_documents(project, &documents, head.as_deref())?;
    Ok(documents.len())
}

fn collect_search_documents_from_source(project: &Project) -> Result<Vec<SearchDocument>> {
    let mut docs = Vec::new();

    for memory in list_memories(project)? {
        docs.push(memory_to_search_document(project, &memory));
    }

    for record in list_records(project)? {
        docs.push(record_to_search_document(project, &record));
    }

    Ok(docs)
}

fn memory_to_search_document(project: &Project, memory: &StoredMemory) -> SearchDocument {
    SearchDocument {
        id: memory.meta.id.clone(),
        source_type: "memory".to_string(),
        kind: memory.meta.kind.clone(),
        title: memory
            .meta
            .subject
            .clone()
            .unwrap_or_else(|| memory.meta.kind.clone()),
        space: memory.meta.space.clone(),
        status: memory.meta.status.clone(),
        path: relative_path(project, &memory.path),
        text: memory.body.clone(),
    }
}

fn record_to_search_document(project: &Project, record: &StoredRecord) -> SearchDocument {
    SearchDocument {
        id: record.meta.id.clone(),
        source_type: "record".to_string(),
        kind: record.meta.schema.clone(),
        title: record.meta.title.clone(),
        space: record.meta.space.clone(),
        status: record.meta.status.clone(),
        path: relative_path(project, &record.path),
        text: record.body.clone(),
    }
}

fn adr_to_search_document(project: &Project, adr: &StoredAdr) -> SearchDocument {
    SearchDocument {
        id: adr.meta.id.clone(),
        source_type: "record".to_string(),
        kind: "adr".to_string(),
        title: adr.meta.title.clone(),
        space: adr.meta.space.clone(),
        status: adr.meta.status.clone(),
        path: relative_path(project, &adr.path),
        text: adr.body.clone(),
    }
}

fn refresh_document_index_if_ready(project: &Project, document: &SearchDocument) -> Result<()> {
    let current_head = current_git_head(project);
    if runtime_index::document_index_ready(project)?
        && runtime_index::document_index_git_head(project)? == current_head
    {
        runtime_index::upsert_document(project, document, current_head.as_deref())?;
    }
    Ok(())
}

pub fn relative_path(project: &Project, path: &Path) -> String {
    path.strip_prefix(&project.root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn validate_memory_kind(kind: &str) -> Result<()> {
    const ALLOWED: &[&str] = &[
        "fact",
        "lesson",
        "observation",
        "failure",
        "preference",
        "state",
        "handoff",
        "warning",
        "decision",
        "note",
    ];

    if ALLOWED.iter().any(|value| value.eq_ignore_ascii_case(kind)) {
        Ok(())
    } else {
        bail!(
            "unsupported memory kind '{}'; allowed: {}",
            kind,
            ALLOWED.join(", ")
        )
    }
}

fn canonical_memory_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_space(space: &str) -> String {
    space.replace('\\', "/").trim_matches('/').to_string()
}

pub fn read_text(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}