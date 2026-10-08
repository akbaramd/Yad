use std::{
    env,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use chrono::{SecondsFormat, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use walkdir::WalkDir;

use crate::{
    atomic,
    config::ProjectConfig,
    schema::{ADR_SCHEMA_YAML, canonical_text_hash},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceMeta {
    pub yad: u32,
    pub id: String,
    pub name: String,
    pub created: String,
}

pub struct ProjectWriteGuard {
    file: File,
}

impl Drop for ProjectWriteGuard {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub yad_dir: PathBuf,
    pub config: ProjectConfig,
}

impl Project {
    pub fn discover(start: Option<&Path>) -> Result<Self> {
        let start = match start {
            Some(path) => path.to_path_buf(),
            None => env::current_dir().context("failed to resolve current directory")?,
        };

        let canonical = start.canonicalize().unwrap_or(start);
        for dir in canonical.ancestors() {
            let config_path = dir.join(".yad").join("project.yaml");
            if config_path.is_file() {
                let raw = fs::read_to_string(&config_path)
                    .with_context(|| format!("failed to read {}", config_path.display()))?;
                let config: ProjectConfig =
                    serde_yaml::from_str(&raw).context("invalid .yad/project.yaml")?;
                return Ok(Self {
                    root: dir.to_path_buf(),
                    yad_dir: dir.join(".yad"),
                    config,
                });
            }
        }

        bail!("not inside a Yad project; run 'yad init' first")
    }

    pub fn init(start: &Path, name: Option<String>) -> Result<Self> {
        let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
        let root = resolve_init_root(&start);
        let yad_dir = root.join(".yad");

        if yad_dir.exists() {
            bail!(
                "{} already exists; this project is already initialized",
                yad_dir.display()
            );
        }

        fs::create_dir_all(yad_dir.join("memory"))?;
        fs::create_dir_all(yad_dir.join("spaces"))?;
        fs::create_dir_all(yad_dir.join("schemas"))?;
        fs::create_dir_all(yad_dir.join(".runtime"))?;

        let project_name = name.unwrap_or_else(|| {
            root.file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("project")
                .to_string()
        });

        let project_id = format!("PRJ-{}", Ulid::new());
        let config = ProjectConfig::new(project_id, project_name);

        atomic::write(
            yad_dir.join("project.yaml"),
            serde_yaml::to_string(&config)?,
        )?;
        atomic::write(
            yad_dir.join("schemas").join("adr.schema.yaml"),
            ADR_SCHEMA_YAML,
        )?;

        let schema_hash = canonical_text_hash(ADR_SCHEMA_YAML);
        atomic::write(
            yad_dir.join("schemas.lock"),
            format!(
                "version: 1\nschemas:\n  adr:\n    version: 1\n    sha256: {}\n",
                schema_hash
            ),
        )?;
        atomic::write(yad_dir.join(".gitignore"), ".runtime/\n.fastembed_cache/\n")?;
        atomic::write(
            yad_dir.join(".gitattributes"),
            "*.md text eol=lf\n*.yaml text eol=lf\n*.yml text eol=lf\n*.json text eol=lf\n.gitignore text eol=lf\n.gitattributes text eol=lf\n",
        )?;
        atomic::write(
            yad_dir.join("README.md"),
            "# Yad project data\n\nThis directory is the project-owned source of truth for Yad memories and structured records.\n\n- memory/: agent-managed project memories\n- spaces/: formal project records grouped by project area\n- schemas/: schema contracts used to validate records\n- .runtime/: local derived state; never commit it\n",
        )?;

        let project = Self {
            root,
            yad_dir,
            config,
        };
        project.ensure_space_named("general", Some("General"))?;
        fs::create_dir_all(project.yad_dir.join("spaces").join("general").join("adr"))?;
        Ok(project)
    }

    pub fn schema_path(&self, schema: &str) -> PathBuf {
        self.yad_dir
            .join("schemas")
            .join(format!("{schema}.schema.yaml"))
    }

    pub fn runtime_dir(&self) -> PathBuf {
        self.yad_dir.join(".runtime")
    }

    pub fn workspace_id(&self) -> Result<String> {
        let runtime = self.runtime_dir();
        fs::create_dir_all(&runtime)?;
        let path = runtime.join("workspace-id");

        if path.is_file() {
            let value = fs::read_to_string(&path)?;
            let value = value.trim();
            if !value.is_empty() {
                return Ok(value.to_string());
            }
        }

        let value = format!("WS-{}", Ulid::new());
        atomic::write(&path, format!("{}\n", value))?;
        Ok(value)
    }

    pub fn qdrant_collection_name(&self) -> Result<String> {
        let workspace_id = self.workspace_id()?;
        Ok(format!(
            "yad_{}_{}",
            sanitize_collection_component(&self.config.id),
            sanitize_collection_component(&workspace_id)
        ))
    }

    pub fn acquire_write_lock(&self, timeout: Duration) -> Result<ProjectWriteGuard> {
        let runtime = self.runtime_dir();
        fs::create_dir_all(&runtime)?;
        let path = runtime.join("write.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("failed to open project write lock {}", path.display()))?;

        let started = Instant::now();
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(ProjectWriteGuard { file }),
                Err(err) if is_lock_contention(&err) => {
                    if started.elapsed() >= timeout {
                        bail!(
                            "project is busy: another Yad write operation holds {}",
                            path.display()
                        );
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("failed to acquire project write lock {}", path.display())
                    });
                }
            }
        }
    }

    pub fn ensure_space(&self, space: &str) -> Result<PathBuf> {
        self.ensure_space_named(space, None)
    }

    pub fn ensure_space_named(&self, space: &str, leaf_name: Option<&str>) -> Result<PathBuf> {
        validate_space(space)?;

        let mut current_id = String::new();
        let mut current_path = self.yad_dir.join("spaces");
        for (index, segment) in space.split('/').enumerate() {
            if !current_id.is_empty() {
                current_id.push('/');
            }
            current_id.push_str(segment);
            current_path.push(segment);
            fs::create_dir_all(&current_path)?;

            let metadata_path = current_path.join(".space.yaml");
            if !metadata_path.exists() {
                let is_leaf = index + 1 == space.split('/').count();
                let name = if is_leaf {
                    leaf_name.unwrap_or(segment)
                } else {
                    segment
                };

                let metadata = SpaceMeta {
                    yad: 1,
                    id: current_id.clone(),
                    name: name.to_string(),
                    created: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
                };
                atomic::write(&metadata_path, serde_yaml::to_string(&metadata)?)?;
            }
        }

        Ok(current_path)
    }

    pub fn get_space(&self, space: &str) -> Result<SpaceMeta> {
        validate_space(space)?;
        let path = self.yad_dir.join("spaces").join(space).join(".space.yaml");
        if !path.is_file() {
            bail!("space '{}' not found", space);
        }
        let raw = fs::read_to_string(&path)?;
        serde_yaml::from_str(&raw).context("invalid .space.yaml")
    }

    pub fn list_spaces(&self) -> Result<Vec<SpaceMeta>> {
        let root = self.yad_dir.join("spaces");
        let mut spaces = Vec::new();

        for entry in WalkDir::new(root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file() && entry.file_name() == ".space.yaml")
        {
            let raw = fs::read_to_string(entry.path())?;
            let metadata: SpaceMeta = serde_yaml::from_str(&raw).context("invalid .space.yaml")?;
            spaces.push(metadata);
        }

        spaces.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(spaces)
    }
}

pub fn validate_space(space: &str) -> Result<()> {
    if space.is_empty() || space.len() > 240 || space.contains('\\') {
        bail!(
            "invalid space '{}'; use lowercase slash-separated identifiers such as facilities/approval",
            space
        );
    }

    for part in space.split('/') {
        if part.is_empty() || part.len() > 64 {
            bail!("invalid space segment in '{}'", space);
        }

        let mut chars = part.chars();
        let Some(first) = chars.next() else {
            bail!("invalid space segment in '{}'", space);
        };

        if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
            bail!(
                "invalid space '{}'; each segment must start with a lowercase letter or digit",
                space
            );
        }

        if !chars.all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '_' | '.')
        }) {
            bail!(
                "invalid space '{}'; allowed characters are a-z, 0-9, '-', '_' and '.'",
                space
            );
        }

        let upper = part.to_ascii_uppercase();
        let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (upper.len() == 4
                && (upper.starts_with("COM") || upper.starts_with("LPT"))
                && upper[3..].chars().all(|ch| ('1'..='9').contains(&ch)));

        if reserved {
            bail!("space segment '{}' is reserved on Windows", part);
        }
    }

    Ok(())
}

fn resolve_init_root(start: &Path) -> PathBuf {
    for dir in start.ancestors() {
        if dir.join(".git").exists() {
            return dir.to_path_buf();
        }
    }
    start.to_path_buf()
}

fn sanitize_collection_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn is_lock_contention(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::WouldBlock || matches!(err.raw_os_error(), Some(32 | 33))
}
