use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use fs2::FileExt;
use reqwest::blocking::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const MODEL_NAME: &str = "multilingual-e5-small-int8";
pub const HF_REPO: &str = "Xenova/multilingual-e5-small";
pub const HF_REVISION: &str = "761b726dd34fb83930e26aab4e9ac3899aa1fa78";

#[derive(Debug, Clone)]
pub struct ModelBundle {
    pub model: PathBuf,
    pub tokenizer: PathBuf,
    pub config: PathBuf,
    pub special_tokens_map: PathBuf,
    pub tokenizer_config: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelStatus {
    pub model: String,
    pub revision: String,
    pub directory: String,
    pub ready: bool,
    pub files: Vec<ModelFileStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelFileStatus {
    pub name: String,
    pub exists: bool,
    pub valid: bool,
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy)]
struct RemoteFile {
    remote: &'static str,
    local: &'static str,
    min_bytes: u64,
    sha256: &'static str,
    json: bool,
}

const FILES: &[RemoteFile] = &[
    RemoteFile {
        remote: "onnx/model_int8.onnx",
        local: "model.onnx",
        min_bytes: 100_000_000,
        sha256: "4d24e2bc01a447951524466ef533e52944bf48509e6552810bcee1a2711cb02c",
        json: false,
    },
    RemoteFile {
        remote: "tokenizer.json",
        local: "tokenizer.json",
        min_bytes: 1_000_000,
        sha256: "0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39",
        json: true,
    },
    RemoteFile {
        remote: "config.json",
        local: "config.json",
        min_bytes: 100,
        sha256: "cb99455288675345e1a4f411438d5d0adbba5fbd3a67ea4fb03c015433b996c1",
        json: true,
    },
    RemoteFile {
        remote: "special_tokens_map.json",
        local: "special_tokens_map.json",
        min_bytes: 100,
        sha256: "d05497f1da52c5e09554c0cd874037a083e1dc1b9cfd48034d1c717f1afc07a7",
        json: true,
    },
    RemoteFile {
        remote: "tokenizer_config.json",
        local: "tokenizer_config.json",
        min_bytes: 100,
        sha256: "a1d6bc8734a6f635dc158508bef000f8e2e5a759c7d92f984b2c86e5ff53425b",
        json: true,
    },
];

#[derive(Debug, Serialize)]
struct Manifest<'a> {
    model: &'a str,
    repo: &'a str,
    revision: &'a str,
}

pub fn ensure_default_bundle() -> Result<ModelBundle> {
    let dir = bundle_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create model cache {}", dir.display()))?;

    let lock_path = dir.join(".download.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open model lock {}", lock_path.display()))?;

    lock.lock_exclusive()
        .context("failed to acquire Yad model download lock")?;

    let result = ensure_files(&dir);
    let _ = FileExt::unlock(&lock);
    result?;

    Ok(bundle_from_dir(&dir))
}

pub fn install_from(source: &Path) -> Result<ModelStatus> {
    let destination = bundle_dir();
    fs::create_dir_all(&destination)?;

    let lock_path = destination.join(".download.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    lock.lock_exclusive()
        .context("failed to acquire Yad model install lock")?;

    let result = (|| {
        for spec in FILES {
            let source_path = source.join(spec.local);
            if !source_path.is_file() {
                bail!("missing required model file {}", source_path.display());
            }
            validate_file(&source_path, *spec)
                .with_context(|| format!("invalid source model file {}", source_path.display()))?;

            let destination_path = destination.join(spec.local);
            let temp = destination_path.with_extension(format!(
                "{}.part.{}",
                destination_path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("tmp"),
                std::process::id()
            ));

            fs::copy(&source_path, &temp).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    temp.display()
                )
            })?;
            validate_file(&temp, *spec)?;

            if destination_path.exists() {
                fs::remove_file(&destination_path)?;
            }
            fs::rename(&temp, &destination_path)?;
        }

        write_manifest(&destination)
    })();

    let _ = FileExt::unlock(&lock);
    result?;
    status()
}

pub fn status() -> Result<ModelStatus> {
    let dir = bundle_dir();
    let mut files = Vec::new();
    let mut ready = true;

    for spec in FILES {
        let path = dir.join(spec.local);
        let exists = path.is_file();
        let bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let valid = exists && validate_file(&path, *spec).is_ok();
        ready &= valid;
        files.push(ModelFileStatus {
            name: spec.local.to_string(),
            exists,
            valid,
            bytes,
        });
    }

    Ok(ModelStatus {
        model: MODEL_NAME.to_string(),
        revision: HF_REVISION.to_string(),
        directory: dir.to_string_lossy().to_string(),
        ready,
        files,
    })
}

fn ensure_files(dir: &Path) -> Result<()> {
    if FILES
        .iter()
        .all(|spec| validate_file(&dir.join(spec.local), *spec).is_ok())
    {
        return write_manifest(dir);
    }

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(900))
        .user_agent(concat!("yad/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to initialize model download client")?;

    for spec in FILES {
        let destination = dir.join(spec.local);
        if validate_file(&destination, *spec).is_ok() {
            continue;
        }

        download_file(&client, *spec, &destination)?;
        validate_file(&destination, *spec)
            .with_context(|| format!("downloaded model file '{}' failed validation", spec.local))?;
    }

    write_manifest(dir)
}

fn write_manifest(dir: &Path) -> Result<()> {
    let manifest = Manifest {
        model: MODEL_NAME,
        repo: HF_REPO,
        revision: HF_REVISION,
    };
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}

fn download_file(client: &Client, spec: RemoteFile, destination: &Path) -> Result<()> {
    let url = format!(
        "https://huggingface.co/{}/resolve/{}/{}",
        HF_REPO, HF_REVISION, spec.remote
    );

    let temp = destination.with_extension(format!(
        "{}.part.{}",
        destination
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("tmp"),
        std::process::id()
    ));

    let _ = fs::remove_file(&temp);

    let mut response = client
        .get(&url)
        .send()
        .with_context(|| format!("failed to download {}", spec.remote))?
        .error_for_status()
        .with_context(|| format!("model server rejected {}", spec.remote))?;

    let expected = response.content_length();
    let mut file =
        File::create(&temp).with_context(|| format!("failed to create {}", temp.display()))?;
    let copied = io::copy(&mut response, &mut file)
        .with_context(|| format!("failed while downloading {}", spec.remote))?;
    file.sync_all()?;

    if let Some(expected) = expected
        && copied != expected
    {
        let _ = fs::remove_file(&temp);
        bail!(
            "incomplete download for {}: expected {} bytes, received {}",
            spec.remote,
            expected,
            copied
        );
    }

    validate_file(&temp, spec)?;

    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(&temp, destination)?;

    Ok(())
}

fn validate_file(path: &Path, spec: RemoteFile) -> Result<()> {
    let metadata = fs::metadata(path).with_context(|| format!("missing {}", path.display()))?;
    if !metadata.is_file() || metadata.len() < spec.min_bytes {
        bail!(
            "{} is too small ({} bytes, expected at least {})",
            path.display(),
            metadata.len(),
            spec.min_bytes
        );
    }

    if spec.json {
        let bytes = fs::read(path)?;
        serde_json::from_slice::<serde_json::Value>(&bytes)
            .with_context(|| format!("{} is not valid JSON", path.display()))?;
    }

    let actual = sha256_file(path)?;
    if actual != spec.sha256 {
        bail!(
            "checksum mismatch for {}: expected {}, got {}",
            path.display(),
            spec.sha256,
            actual
        );
    }

    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut HashWriter(&mut hasher))?;
    Ok(format!("{:x}", hasher.finalize()))
}

struct HashWriter<'a>(&'a mut Sha256);

impl io::Write for HashWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn bundle_from_dir(dir: &Path) -> ModelBundle {
    ModelBundle {
        model: dir.join("model.onnx"),
        tokenizer: dir.join("tokenizer.json"),
        config: dir.join("config.json"),
        special_tokens_map: dir.join("special_tokens_map.json"),
        tokenizer_config: dir.join("tokenizer_config.json"),
    }
}

pub fn bundle_dir() -> PathBuf {
    model_cache_root().join(MODEL_NAME).join(HF_REVISION)
}

fn model_cache_root() -> PathBuf {
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        PathBuf::from(base).join("Yad").join("models")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
            .join(".cache")
            .join("yad")
            .join("models")
    } else {
        std::env::temp_dir().join("yad").join("models")
    }
}
