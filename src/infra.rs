use std::process::Command;

use anyhow::{Context, Result, bail};

pub const QDRANT_CONTAINER: &str = "yad-qdrant";
pub const QDRANT_IMAGE: &str = "qdrant/qdrant:v1.19.1";
pub const QDRANT_VOLUME: &str = "yad-qdrant-storage";

pub fn docker_available() -> bool {
    Command::new("docker")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn qdrant_container_state() -> Result<Option<String>> {
    if !docker_available() {
        return Ok(None);
    }

    let output = Command::new("docker")
        .args([
            "ps",
            "-a",
            "--filter",
            &format!("name=^/{}$", QDRANT_CONTAINER),
            "--format",
            "{{.State}}",
        ])
        .output()
        .context("failed to inspect Qdrant container")?;

    if !output.status.success() {
        bail!("docker ps failed");
    }

    let state = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if state.is_empty() {
        Ok(None)
    } else {
        Ok(Some(state))
    }
}

pub fn up() -> Result<String> {
    if !docker_available() {
        bail!("Docker is not available");
    }

    match qdrant_container_state()? {
        Some(state) if state.eq_ignore_ascii_case("running") => {
            Ok("Qdrant is already running".to_string())
        }
        Some(_) => {
            let status = Command::new("docker")
                .args(["start", QDRANT_CONTAINER])
                .status()
                .context("failed to start Qdrant container")?;
            if !status.success() {
                bail!("docker start failed");
            }
            Ok("Qdrant container started".to_string())
        }
        None => {
            let status = Command::new("docker")
                .args([
                    "run",
                    "-d",
                    "--name",
                    QDRANT_CONTAINER,
                    "-p",
                    "127.0.0.1:6333:6333",
                    "-p",
                    "127.0.0.1:6334:6334",
                    "-v",
                    &format!("{}:/qdrant/storage", QDRANT_VOLUME),
                    QDRANT_IMAGE,
                ])
                .status()
                .context("failed to create Qdrant container")?;
            if !status.success() {
                bail!("docker run failed");
            }
            Ok(format!("Qdrant {} started on localhost", QDRANT_IMAGE))
        }
    }
}

pub fn down() -> Result<String> {
    match qdrant_container_state()? {
        None => Ok("Qdrant container does not exist".to_string()),
        Some(state) if !state.eq_ignore_ascii_case("running") => {
            Ok("Qdrant is already stopped".to_string())
        }
        Some(_) => {
            let status = Command::new("docker")
                .args(["stop", QDRANT_CONTAINER])
                .status()
                .context("failed to stop Qdrant container")?;
            if !status.success() {
                bail!("docker stop failed");
            }
            Ok("Qdrant stopped".to_string())
        }
    }
}
