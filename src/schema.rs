use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{frontmatter, model::AdrMeta};

pub const ADR_SCHEMA_YAML: &str = r#"id: adr
version: 1
title: Architecture Decision Record
id_prefix: ADR
directory: adr
initial_status: proposed
statuses:
  - proposed
  - accepted
  - rejected
  - deprecated
  - superseded
transitions:
  proposed:
    - accepted
    - rejected
  accepted:
    - deprecated
    - superseded
  rejected: []
  deprecated: []
  superseded: []
sections:
  - key: context
    title: Context
    required: true
  - key: decision
    title: Decision
    required: true
  - key: consequences
    title: Consequences
    required: true
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinition {
    pub id: String,
    pub version: u32,
    pub title: String,
    pub id_prefix: String,
    pub directory: String,
    pub initial_status: String,
    pub statuses: Vec<String>,
    pub transitions: BTreeMap<String, Vec<String>>,
    pub sections: Vec<SchemaSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaSection {
    pub key: String,
    pub title: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    pub id: Option<String>,
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
}

pub fn load(path: &Path) -> Result<SchemaDefinition> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read schema {}", path.display()))?;
    serde_yaml::from_str(&raw).context("invalid schema YAML")
}

pub fn validate_adr(path: &Path, schema: &SchemaDefinition) -> Result<ValidationReport> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;

    let (meta, body) = match frontmatter::parse::<AdrMeta>(&raw) {
        Ok(v) => v,
        Err(err) => {
            return Ok(ValidationReport {
                id: None,
                valid: false,
                issues: vec![ValidationIssue {
                    code: "frontmatter.invalid".to_string(),
                    message: err.to_string(),
                }],
            });
        }
    };

    let mut issues = Vec::new();

    if meta.schema != schema.id {
        issues.push(ValidationIssue {
            code: "schema.mismatch".to_string(),
            message: format!("expected schema '{}', got '{}'", schema.id, meta.schema),
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

    if !schema.statuses.iter().any(|s| s == &meta.status) {
        issues.push(ValidationIssue {
            code: "status.invalid".to_string(),
            message: format!("invalid ADR status '{}'", meta.status),
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

    for section in &schema.sections {
        if !section.required {
            continue;
        }
        let content = extract_section(&body, &section.title);
        match content {
            None => issues.push(ValidationIssue {
                code: format!("section.{}.missing", section.key),
                message: format!("missing required section '## {}'", section.title),
            }),
            Some(value) if value.trim().is_empty() || value.contains("<!-- required -->") => issues
                .push(ValidationIssue {
                    code: format!("section.{}.empty", section.key),
                    message: format!("required section '{}' is empty", section.title),
                }),
            _ => {}
        }
    }

    Ok(ValidationReport {
        id: Some(meta.id),
        valid: issues.is_empty(),
        issues,
    })
}

pub fn ensure_transition(schema: &SchemaDefinition, from: &str, to: &str) -> Result<()> {
    let allowed = schema
        .transitions
        .get(from)
        .ok_or_else(|| anyhow::anyhow!("unknown current status '{from}'"))?;

    if allowed.iter().any(|s| s == to) {
        Ok(())
    } else {
        bail!("invalid status transition: {from} -> {to}")
    }
}

pub fn extract_section<'a>(body: &'a str, title: &str) -> Option<&'a str> {
    let heading = format!("## {}", title);
    let start = body.find(&heading)?;
    let after_heading = &body[start + heading.len()..];
    let content_start = after_heading.find('\n').map(|n| n + 1).unwrap_or(0);
    let content = &after_heading[content_start..];

    if let Some(next) = content.find("\n## ") {
        Some(content[..next].trim())
    } else {
        Some(content.trim())
    }
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
    let lock: SchemaLockFile = serde_yaml::from_str(&raw).context("invalid .yad/schemas.lock")?;

    let mut statuses = Vec::new();
    for (id, entry) in lock.schemas {
        let schema_path = yad_dir.join("schemas").join(format!("{}.schema.yaml", id));

        if !schema_path.is_file() {
            statuses.push(SchemaLockStatus {
                id,
                version: entry.version,
                expected_sha256: entry.sha256,
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
        let hash_matches = actual.eq_ignore_ascii_case(&entry.sha256);

        let message = if parsed.is_err() {
            Some("schema YAML is invalid".to_string())
        } else if !version_matches {
            Some("schema version does not match schemas.lock".to_string())
        } else if !hash_matches {
            Some("schema content hash does not match schemas.lock".to_string())
        } else {
            None
        };

        statuses.push(SchemaLockStatus {
            id,
            version: entry.version,
            expected_sha256: entry.sha256,
            actual_sha256: Some(actual),
            valid: version_matches && hash_matches,
            message,
        });
    }

    Ok(SchemaLockReport {
        valid: statuses.iter().all(|item| item.valid),
        schemas: statuses,
    })
}

pub fn canonical_text_hash(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    format!("{:x}", Sha256::digest(normalized.as_bytes()))
}
