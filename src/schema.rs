use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::{
    atomic, builtin_schemas, frontmatter,
    model::RecordMeta,
};

pub const ADR_SCHEMA_YAML: &str = builtin_schemas::ADR_SCHEMA_YAML;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinition {
    pub id: String,
    pub version: u32,
    pub title: String,
    #[serde(default)]
    pub abbreviation: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standard: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub id_prefix: String,
    pub directory: String,
    pub initial_status: String,
    pub statuses: Vec<String>,
    #[serde(default)]
    pub historical_statuses: Vec<String>,
    #[serde(default)]
    pub authoritative_statuses: Vec<String>,
    pub transitions: BTreeMap<String, Vec<String>>,
    pub sections: Vec<SchemaSection>,
}

impl SchemaDefinition {
    pub fn abbreviation(&self) -> &str {
        if self.abbreviation.trim().is_empty() {
            &self.id_prefix
        } else {
            &self.abbreviation
        }
    }

    pub fn is_historical_status(&self, status: &str) -> bool {
        self.historical_statuses
            .iter()
            .any(|value| value.eq_ignore_ascii_case(status))
    }

    pub fn is_authoritative_status(&self, status: &str) -> bool {
        self.authoritative_statuses
            .iter()
            .any(|value| value.eq_ignore_ascii_case(status))
    }

    pub fn section(&self, key: &str) -> Option<&SchemaSection> {
        self.sections
            .iter()
            .find(|section| section.key.eq_ignore_ascii_case(key))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaSection {
    pub key: String,
    pub title: String,
    pub required: bool,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    pub id: Option<String>,
    pub schema: Option<String>,
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaInstallReport {
    pub installed: Vec<String>,
    pub updated: Vec<String>,
    pub skipped: Vec<String>,
    pub total_builtins: usize,
}

pub fn load(path: &Path) -> Result<SchemaDefinition> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read schema {}", path.display()))?;
    parse_str(&raw).with_context(|| format!("invalid schema {}", path.display()))
}

pub fn parse_str(raw: &str) -> Result<SchemaDefinition> {
    let definition: SchemaDefinition =
        serde_yaml::from_str(raw).context("invalid schema YAML")?;
    validate_definition(&definition)?;
    Ok(definition)
}

pub fn validate_definition(schema: &SchemaDefinition) -> Result<()> {
    if schema.version == 0 {
        bail!("schema '{}' version must be greater than zero", schema.id);
    }
    if schema.id.trim().is_empty()
        || !schema
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        bail!(
            "schema id '{}' must use lowercase letters, digits, and '-' only",
            schema.id
        );
    }
    if schema.title.trim().is_empty() {
        bail!("schema '{}' title cannot be empty", schema.id);
    }
    if schema.id_prefix.trim().is_empty()
        || schema.id_prefix.len() > 16
        || !schema
            .id_prefix
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
    {
        bail!(
            "schema '{}' id_prefix '{}' must use uppercase letters, digits, and '-' only (max 16 chars)",
            schema.id,
            schema.id_prefix
        );
    }
    validate_directory(&schema.directory)
        .with_context(|| format!("schema '{}' has invalid directory", schema.id))?;

    if schema.statuses.is_empty() {
        bail!("schema '{}' must define at least one status", schema.id);
    }

    let mut statuses = HashSet::new();
    for status in &schema.statuses {
        let normalized = status.trim().to_ascii_lowercase();
        if normalized.is_empty()
            || !normalized
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            bail!(
                "schema '{}' has invalid status '{}'",
                schema.id,
                status
            );
        }
        if !statuses.insert(normalized) {
            bail!("schema '{}' has duplicate status '{}'", schema.id, status);
        }
    }

    if !schema
        .statuses
        .iter()
        .any(|status| status == &schema.initial_status)
    {
        bail!(
            "schema '{}' initial_status '{}' is not present in statuses",
            schema.id,
            schema.initial_status
        );
    }

    for status in &schema.statuses {
        if !schema.transitions.contains_key(status) {
            bail!(
                "schema '{}' must define transitions for status '{}'",
                schema.id,
                status
            );
        }
    }
    for (from, targets) in &schema.transitions {
        if !schema.statuses.iter().any(|status| status == from) {
            bail!(
                "schema '{}' transition source '{}' is not a declared status",
                schema.id,
                from
            );
        }
        for target in targets {
            if !schema.statuses.iter().any(|status| status == target) {
                bail!(
                    "schema '{}' transition '{} -> {}' targets an unknown status",
                    schema.id,
                    from,
                    target
                );
            }
        }
    }

    for status in schema
        .historical_statuses
        .iter()
        .chain(schema.authoritative_statuses.iter())
    {
        if !schema.statuses.iter().any(|declared| declared == status) {
            bail!(
                "schema '{}' classifies unknown status '{}'",
                schema.id,
                status
            );
        }
    }

    let historical = schema
        .historical_statuses
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    for status in &schema.authoritative_statuses {
        if historical.contains(&status.to_ascii_lowercase()) {
            bail!(
                "schema '{}' status '{}' cannot be both historical and authoritative",
                schema.id,
                status
            );
        }
    }

    let mut keys = HashSet::new();
    let mut titles = HashSet::new();
    for section in &schema.sections {
        if section.key.trim().is_empty()
            || !section
                .key
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        {
            bail!(
                "schema '{}' has invalid section key '{}'",
                schema.id,
                section.key
            );
        }
        if section.title.trim().is_empty() {
            bail!(
                "schema '{}' section '{}' has an empty title",
                schema.id,
                section.key
            );
        }
        if !keys.insert(section.key.to_ascii_lowercase()) {
            bail!(
                "schema '{}' has duplicate section key '{}'",
                schema.id,
                section.key
            );
        }
        if !titles.insert(section.title.to_ascii_lowercase()) {
            bail!(
                "schema '{}' has duplicate section title '{}'",
                schema.id,
                section.title
            );
        }
    }

    Ok(())
}

fn validate_directory(directory: &str) -> Result<()> {
    let value = directory.trim();
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
    {
        bail!(
            "directory '{}' must be a single safe relative path segment",
            directory
        );
    }
    Ok(())
}

pub fn list_definitions(yad_dir: &Path) -> Result<Vec<SchemaDefinition>> {
    let root = yad_dir.join("schemas");
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut definitions = Vec::new();
    for entry in WalkDir::new(&root).max_depth(1) {
        let entry = entry.context("failed while traversing .yad/schemas")?;
        if !entry.file_type().is_file() || !is_schema_file(entry.path()) {
            continue;
        }
        let definition = load(entry.path())?;
        let expected_name = format!("{}.schema.yaml", definition.id);
        if entry.file_name().to_string_lossy() != expected_name {
            bail!(
                "schema '{}' must be stored as schemas/{}",
                definition.id,
                expected_name
            );
        }
        definitions.push(definition);
    }
    definitions.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(definitions)
}

pub fn resolve(yad_dir: &Path, selector: &str) -> Result<SchemaDefinition> {
    let selector = selector.trim();
    if selector.is_empty() {
        bail!("schema selector cannot be empty");
    }

    let definitions = list_definitions(yad_dir)?;

    if let Some(schema) = definitions
        .iter()
        .find(|schema| schema.id.eq_ignore_ascii_case(selector))
    {
        return Ok(schema.clone());
    }

    let mut matches = definitions
        .into_iter()
        .filter(|schema| {
            schema.abbreviation().eq_ignore_ascii_case(selector)
                || schema.id_prefix.eq_ignore_ascii_case(selector)
                || schema
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(selector))
        })
        .collect::<Vec<_>>();

    match matches.len() {
        0 => bail!("schema or abbreviation '{}' not found", selector),
        1 => Ok(matches.remove(0)),
        _ => {
            let ids = matches
                .iter()
                .map(|schema| schema.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "schema selector '{}' is ambiguous; matches: {}",
                selector,
                ids
            )
        }
    }
}

pub fn validate_record(path: &Path, schema: &SchemaDefinition) -> Result<ValidationReport> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;

    let (meta, body) = match frontmatter::parse::<RecordMeta>(&raw) {
        Ok(value) => value,
        Err(err) => {
            return Ok(ValidationReport {
                id: None,
                schema: None,
                valid: false,
                issues: vec![ValidationIssue {
                    code: "frontmatter.invalid".to_string(),
                    message: err.to_string(),
                }],
            });
        }
    };

    let mut issues = Vec::new();

    if meta.yad != 1 {
        issues.push(ValidationIssue {
            code: "yad.version".to_string(),
            message: format!("unsupported yad document version {}", meta.yad),
        });
    }
    if meta.schema != schema.id {
        issues.push(ValidationIssue {
            code: "schema.mismatch".to_string(),
            message: format!(
                "expected schema '{}', got '{}'",
                schema.id, meta.schema
            ),
        });
    }
    if meta.schema_version != schema.version {
        issues.push(ValidationIssue {
            code: "schema.version".to_string(),
            message: format!(
                "expected schema version {}, got {}",
                schema.version, meta.schema_version
            ),
        });
    }
    if !meta.id.starts_with(&format!("{}-", schema.id_prefix)) {
        issues.push(ValidationIssue {
            code: "id.prefix".to_string(),
            message: format!(
                "record id '{}' must start with '{}-'",
                meta.id, schema.id_prefix
            ),
        });
    }
    if !schema.statuses.iter().any(|status| status == &meta.status) {
        issues.push(ValidationIssue {
            code: "status.invalid".to_string(),
            message: format!(
                "invalid '{}' status '{}'",
                schema.id, meta.status
            ),
        });
    }
    if meta.title.trim().is_empty() {
        issues.push(ValidationIssue {
            code: "title.required".to_string(),
            message: "title cannot be empty".to_string(),
        });
    }
    if meta.space.trim().is_empty() {
        issues.push(ValidationIssue {
            code: "space.required".to_string(),
            message: "space cannot be empty".to_string(),
        });
    }
    if DateTime::parse_from_rfc3339(&meta.created).is_err() {
        issues.push(ValidationIssue {
            code: "created.invalid".to_string(),
            message: "created must be an RFC3339 timestamp".to_string(),
        });
    }
    if DateTime::parse_from_rfc3339(&meta.updated).is_err() {
        issues.push(ValidationIssue {
            code: "updated.invalid".to_string(),
            message: "updated must be an RFC3339 timestamp".to_string(),
        });
    }
    if meta.status == "superseded" && meta.superseded_by.is_empty() {
        issues.push(ValidationIssue {
            code: "superseded_by.required".to_string(),
            message: "superseded record must reference its replacement".to_string(),
        });
    }

    for section in &schema.sections {
        if !section.required {
            continue;
        }

        match extract_section(&body, &section.title) {
            None => issues.push(ValidationIssue {
                code: format!("section.{}.missing", section.key),
                message: format!("missing required section '## {}'", section.title),
            }),
            Some(value)
                if value.trim().is_empty()
                    || value.contains("<!-- required -->")
                    || value.contains(&format!("<!-- required: {} -->", section.key)) =>
            {
                issues.push(ValidationIssue {
                    code: format!("section.{}.empty", section.key),
                    message: format!("required section '{}' is empty", section.title),
                });
            }
            _ => {}
        }
    }

    Ok(ValidationReport {
        id: Some(meta.id),
        schema: Some(meta.schema),
        valid: issues.is_empty(),
        issues,
    })
}

pub fn validate_adr(path: &Path, schema: &SchemaDefinition) -> Result<ValidationReport> {
    validate_record(path, schema)
}

pub fn ensure_transition(schema: &SchemaDefinition, from: &str, to: &str) -> Result<()> {
    let allowed = schema
        .transitions
        .get(from)
        .ok_or_else(|| anyhow::anyhow!("unknown current status '{from}'"))?;

    if allowed.iter().any(|status| status == to) {
        Ok(())
    } else {
        bail!(
            "invalid {} lifecycle transition: {} -> {}",
            schema.id,
            from,
            to
        )
    }
}

pub fn extract_section<'a>(body: &'a str, title: &str) -> Option<&'a str> {
    let heading = format!("## {}", title);
    let mut byte_offset = 0usize;

    for line in body.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
        let next_offset = byte_offset + line.len();

        if trimmed == heading {
            let remaining = &body[next_offset..];
            let mut end = remaining.len();
            let mut relative = 0usize;

            for candidate in remaining.split_inclusive('\n') {
                if candidate
                    .trim_end_matches(&['\r', '\n'][..])
                    .starts_with("## ")
                {
                    end = relative;
                    break;
                }
                relative += candidate.len();
            }

            return Some(remaining[..end].trim());
        }

        byte_offset = next_offset;
    }

    None
}

pub fn replace_section(body: &str, title: &str, value: &str) -> String {
    let normalized = body.replace("\r\n", "\n");
    let heading = format!("## {}", title);
    let lines = normalized.lines().collect::<Vec<_>>();
    let Some(start_line) = lines.iter().position(|line| *line == heading) else {
        let mut result = normalized.trim_end().to_string();
        result.push_str(&format!("\n\n{}\n\n{}\n", heading, value.trim()));
        return result;
    };

    let end_line = lines
        .iter()
        .enumerate()
        .skip(start_line + 1)
        .find_map(|(index, line)| line.starts_with("## ").then_some(index))
        .unwrap_or(lines.len());

    let mut result = Vec::new();
    result.extend_from_slice(&lines[..=start_line]);
    result.push("");
    if !value.trim().is_empty() {
        result.extend(value.trim().lines());
    }
    if end_line < lines.len() {
        result.push("");
        result.extend_from_slice(&lines[end_line..]);
    }

    let mut rendered = result.join("\n");
    rendered.push('\n');
    rendered
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaLockFile {
    pub version: u32,
    pub schemas: BTreeMap<String, SchemaLockEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaLockEntry {
    pub version: u32,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaLockReport {
    pub valid: bool,
    pub schemas: Vec<SchemaLockStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaLockStatus {
    pub id: String,
    pub version: u32,
    pub expected_sha256: String,
    pub actual_sha256: Option<String>,
    pub valid: bool,
    pub message: Option<String>,
}

pub fn verify_lock(yad_dir: &Path) -> Result<SchemaLockReport> {
    let lock_path = yad_dir.join("schemas.lock");
    let raw = fs::read_to_string(&lock_path)
        .with_context(|| format!("failed to read {}", lock_path.display()))?;
    let lock: SchemaLockFile =
        serde_yaml::from_str(&raw).context("invalid .yad/schemas.lock")?;

    let schema_dir = yad_dir.join("schemas");
    let mut statuses = Vec::new();
    let mut locked_ids = HashSet::new();

    for (id, entry) in &lock.schemas {
        locked_ids.insert(id.clone());
        let schema_path = schema_dir.join(format!("{}.schema.yaml", id));

        if !schema_path.is_file() {
            statuses.push(SchemaLockStatus {
                id: id.clone(),
                version: entry.version,
                expected_sha256: entry.sha256.clone(),
                actual_sha256: None,
                valid: false,
                message: Some("schema file is missing".to_string()),
            });
            continue;
        }

        let schema_text = fs::read_to_string(&schema_path)?;
        let actual = canonical_text_hash(&schema_text);
        let parsed = load(&schema_path);
        let version_matches = parsed
            .as_ref()
            .map(|schema| schema.version == entry.version)
            .unwrap_or(false);
        let id_matches = parsed
            .as_ref()
            .map(|schema| schema.id == *id)
            .unwrap_or(false);
        let hash_matches = actual.eq_ignore_ascii_case(&entry.sha256);

        let message = if parsed.is_err() {
            Some("schema YAML or definition is invalid".to_string())
        } else if !id_matches {
            Some("schema id does not match schemas.lock entry".to_string())
        } else if !version_matches {
            Some("schema version does not match schemas.lock".to_string())
        } else if !hash_matches {
            Some("schema content hash does not match schemas.lock".to_string())
        } else {
            None
        };

        statuses.push(SchemaLockStatus {
            id: id.clone(),
            version: entry.version,
            expected_sha256: entry.sha256.clone(),
            actual_sha256: Some(actual),
            valid: id_matches && version_matches && hash_matches,
            message,
        });
    }

    if schema_dir.exists() {
        for entry in WalkDir::new(&schema_dir).max_depth(1) {
            let entry = entry.context("failed while traversing .yad/schemas")?;
            if !entry.file_type().is_file() || !is_schema_file(entry.path()) {
                continue;
            }
            let definition = match load(entry.path()) {
                Ok(definition) => definition,
                Err(err) => {
                    statuses.push(SchemaLockStatus {
                        id: entry.file_name().to_string_lossy().to_string(),
                        version: 0,
                        expected_sha256: String::new(),
                        actual_sha256: fs::read_to_string(entry.path())
                            .ok()
                            .map(|text| canonical_text_hash(&text)),
                        valid: false,
                        message: Some(err.to_string()),
                    });
                    continue;
                }
            };
            if !locked_ids.contains(&definition.id) {
                let text = fs::read_to_string(entry.path())?;
                statuses.push(SchemaLockStatus {
                    id: definition.id,
                    version: definition.version,
                    expected_sha256: String::new(),
                    actual_sha256: Some(canonical_text_hash(&text)),
                    valid: false,
                    message: Some(
                        "schema exists on disk but is not pinned in schemas.lock".to_string(),
                    ),
                });
            }
        }
    }

    statuses.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(SchemaLockReport {
        valid: statuses.iter().all(|item| item.valid),
        schemas: statuses,
    })
}

pub fn refresh_lock(yad_dir: &Path) -> Result<SchemaLockFile> {
    let schema_dir = yad_dir.join("schemas");
    fs::create_dir_all(&schema_dir)?;

    let mut schemas = BTreeMap::new();
    for definition in list_definitions(yad_dir)? {
        let path = schema_dir.join(format!("{}.schema.yaml", definition.id));
        let text = fs::read_to_string(&path)?;
        schemas.insert(
            definition.id,
            SchemaLockEntry {
                version: definition.version,
                sha256: canonical_text_hash(&text),
            },
        );
    }

    let lock = SchemaLockFile {
        version: 1,
        schemas,
    };
    atomic::write(
        yad_dir.join("schemas.lock"),
        serde_yaml::to_string(&lock)?,
    )?;
    Ok(lock)
}

pub fn install_builtins(yad_dir: &Path, force: bool) -> Result<SchemaInstallReport> {
    let schema_dir = yad_dir.join("schemas");
    fs::create_dir_all(&schema_dir)?;

    let lock_path = yad_dir.join("schemas.lock");
    if lock_path.exists() && !force {
        let report = verify_lock(yad_dir)?;
        if !report.valid {
            bail!(
                "schema lock is not valid; fix the project or rerun schema upgrade with --force"
            );
        }
    }

    let mut installed = Vec::new();
    let mut updated = Vec::new();
    let mut skipped = Vec::new();

    for builtin in builtin_schemas::BUILTIN_SCHEMAS {
        let definition = parse_str(builtin.yaml)
            .with_context(|| format!("invalid built-in schema '{}'", builtin.id))?;
        if definition.id != builtin.id {
            bail!(
                "built-in schema catalog mismatch: '{}' declares '{}'",
                builtin.id,
                definition.id
            );
        }

        let path = schema_dir.join(format!("{}.schema.yaml", builtin.id));
        if !path.exists() {
            atomic::write(&path, builtin.yaml)?;
            installed.push(builtin.id.to_string());
            continue;
        }

        let existing = fs::read_to_string(&path)?;
        if canonical_text_hash(&existing) == canonical_text_hash(builtin.yaml) {
            skipped.push(builtin.id.to_string());
        } else if force {
            atomic::write(&path, builtin.yaml)?;
            updated.push(builtin.id.to_string());
        } else {
            skipped.push(builtin.id.to_string());
        }
    }

    refresh_lock(yad_dir)?;

    Ok(SchemaInstallReport {
        installed,
        updated,
        skipped,
        total_builtins: builtin_schemas::count(),
    })
}

pub fn import_schema(yad_dir: &Path, source: &Path, force: bool) -> Result<SchemaDefinition> {
    let raw = fs::read_to_string(source)
        .with_context(|| format!("failed to read schema source {}", source.display()))?;
    let definition = parse_str(&raw)?;
    let destination = yad_dir
        .join("schemas")
        .join(format!("{}.schema.yaml", definition.id));

    if destination.exists() && !force {
        bail!(
            "schema '{}' already exists; use --force to replace it",
            definition.id
        );
    }

    atomic::write(&destination, raw)?;
    refresh_lock(yad_dir)?;
    Ok(definition)
}

pub fn canonical_text_hash(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    format!("{:x}", Sha256::digest(normalized.as_bytes()))
}

fn is_schema_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(|name| name.ends_with(".schema.yaml"))
        .unwrap_or(false)
}

pub fn schema_path(yad_dir: &Path, id: &str) -> PathBuf {
    yad_dir
        .join("schemas")
        .join(format!("{}.schema.yaml", id))
}
