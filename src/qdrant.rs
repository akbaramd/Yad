use std::{
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::project::Project;

#[derive(Debug, Clone)]
pub struct QdrantStore {
    client: Client,
    base_url: String,
    collection: String,
    dimensions: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct VectorPoint {
    pub id: String,
    pub vector: Vec<f32>,
    pub payload: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScoredPoint {
    pub score: f32,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    result: Vec<ScoredPoint>,
}

impl QdrantStore {
    pub fn for_project(project: &Project) -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(30))
            .build()
            .context("failed to initialize HTTP client")?;

        Ok(Self {
            client,
            base_url: project.config.qdrant.url.trim_end_matches('/').to_string(),
            collection: project.qdrant_collection_name()?,
            dimensions: project.config.embedding.dimensions,
        })
    }

    pub fn is_available(&self) -> bool {
        self.client
            .get(format!("{}/collections", self.base_url))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    pub fn collection_exists(&self) -> Result<bool> {
        let response = self
            .client
            .get(format!("{}/collections/{}", self.base_url, self.collection))
            .send();

        match response {
            Ok(r) if r.status().is_success() => Ok(true),
            Ok(r) if r.status().as_u16() == 404 => Ok(false),
            Ok(r) => bail!("Qdrant returned {}", r.status()),
            Err(err) => Err(err).context("cannot connect to Qdrant"),
        }
    }

    pub fn ensure_collection(&self) -> Result<()> {
        if self.collection_exists()? {
            return Ok(());
        }
        self.create_collection()?;
        self.wait_for_collection_state(true, Duration::from_secs(3))
    }

    pub fn recreate_collection(&self) -> Result<()> {
        if self.collection_exists()? {
            let response = self
                .client
                .delete(format!("{}/collections/{}", self.base_url, self.collection))
                .send()
                .context("failed to delete Qdrant collection")?;

            if !response.status().is_success() {
                bail!("failed to delete Qdrant collection: {}", response.status());
            }

            self.wait_for_collection_state(false, Duration::from_secs(3))?;
        }

        self.create_collection()?;
        self.wait_for_collection_state(true, Duration::from_secs(3))
    }

    fn create_collection(&self) -> Result<()> {
        let response = self
            .client
            .put(format!("{}/collections/{}", self.base_url, self.collection))
            .json(&json!({
                "vectors": {
                    "size": self.dimensions,
                    "distance": "Cosine"
                }
            }))
            .send()
            .context("failed to create Qdrant collection")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            bail!("failed to create Qdrant collection: {} {}", status, text);
        }

        Ok(())
    }

    pub fn upsert(&self, points: &[VectorPoint]) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }

        let mut last_error = None;

        for attempt in 0..5 {
            let response = self
                .client
                .put(format!(
                    "{}/collections/{}/points?wait=true",
                    self.base_url, self.collection
                ))
                .json(&json!({ "points": points }))
                .send()
                .context("failed to upsert Qdrant points")?;

            if response.status().is_success() {
                return Ok(());
            }

            let status = response.status();
            let text = response.text().unwrap_or_default();

            if status.as_u16() == 404 && attempt < 4 {
                last_error = Some(format!("{} {}", status, text));
                thread::sleep(Duration::from_millis(100 * (attempt + 1) as u64));
                continue;
            }

            bail!("Qdrant upsert failed: {} {}", status, text);
        }

        bail!(
            "Qdrant upsert failed after retries: {}",
            last_error.unwrap_or_else(|| "unknown error".to_string())
        )
    }

    pub fn delete_source(&self, source_id: &str) -> Result<()> {
        if !self.collection_exists()? {
            return Ok(());
        }

        let response = self
            .client
            .post(format!(
                "{}/collections/{}/points/delete?wait=true",
                self.base_url, self.collection
            ))
            .json(&json!({
                "filter": {
                    "must": [
                        {
                            "key": "source_id",
                            "match": { "value": source_id }
                        }
                    ]
                }
            }))
            .send()
            .context("failed to delete old Qdrant points")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            bail!("Qdrant delete failed: {} {}", status, text);
        }

        Ok(())
    }

    pub fn search(
        &self,
        vector: &[f32],
        limit: usize,
        filter: Option<&Value>,
    ) -> Result<Vec<ScoredPoint>> {
        let hnsw_ef = limit.saturating_mul(2).clamp(256, 4096);
        let mut request = json!({
            "vector": vector,
            "limit": limit,
            "with_payload": true,
            "params": {
                "hnsw_ef": hnsw_ef
            }
        });

        if let Some(filter) = filter {
            request["filter"] = filter.clone();
        }

        let response = self
            .client
            .post(format!(
                "{}/collections/{}/points/search",
                self.base_url, self.collection
            ))
            .json(&request)
            .send()
            .context("failed to query Qdrant")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            bail!("Qdrant search failed: {} {}", status, text);
        }

        let parsed: SearchResponse = response.json().context("invalid Qdrant search response")?;
        Ok(parsed.result)
    }

    fn wait_for_collection_state(&self, should_exist: bool, timeout: Duration) -> Result<()> {
        let started = Instant::now();

        loop {
            match self.collection_exists() {
                Ok(exists) if exists == should_exist => return Ok(()),
                Ok(_) => {}
                Err(err) => {
                    if started.elapsed() >= timeout {
                        return Err(err).context("Qdrant collection readiness check failed");
                    }
                }
            }

            if started.elapsed() >= timeout {
                bail!(
                    "timed out waiting for Qdrant collection '{}' to become {}",
                    self.collection,
                    if should_exist { "ready" } else { "deleted" }
                );
            }

            thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn collection_name(&self) -> &str {
        &self.collection
    }
}
