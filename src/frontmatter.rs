use anyhow::{Context, Result, bail};
use serde::{Serialize, de::DeserializeOwned};

pub fn parse<T: DeserializeOwned>(input: &str) -> Result<(T, String)> {
    let (yaml, body) = split(input)?;
    let metadata = serde_yaml::from_str::<T>(&yaml).context("invalid YAML front matter")?;
    Ok((metadata, body))
}

pub fn render<T: Serialize>(metadata: &T, body: &str) -> Result<String> {
    let yaml = serde_yaml::to_string(metadata).context("failed to serialize YAML front matter")?;
    Ok(format!("---\n{}---\n\n{}\n", yaml, body.trim_end()))
}

pub fn split(input: &str) -> Result<(String, String)> {
    let normalized = input.replace("\r\n", "\n");
    let mut lines = normalized.lines();

    if lines.next() != Some("---") {
        bail!("document is missing YAML front matter");
    }

    let mut yaml_lines = Vec::new();
    let mut found_end = false;
    let mut body_lines = Vec::new();

    for line in lines {
        if !found_end {
            if line.trim() == "---" {
                found_end = true;
            } else {
                yaml_lines.push(line);
            }
        } else {
            body_lines.push(line);
        }
    }

    if !found_end {
        bail!("document front matter is not closed with ---");
    }

    Ok((
        yaml_lines.join("\n"),
        body_lines.join("\n").trim_start().to_string(),
    ))
}
