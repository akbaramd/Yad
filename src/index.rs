use std::fs;

use anyhow::{Context, Result};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    atomic,
    embedding::EmbeddingService,
    model::SearchDocument,
    model_cache,
    project::Project,
    qdrant::{QdrantStore, VectorPoint},
    schema, storage,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexState {
    pub fingerprint: String,
    pub indexed_at: String,
    pub documents: usize,
    pub chunks: usize,
    pub collection: String,
    #[serde(default)]
    pub embedding_model: String,
    #[serde(default)]
    pub embedding_dimensions: usize,
    #[serde(default)]
    pub embedding_revision: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexStatus {
    pub has_state: bool,
    pub stale: bool,
    pub current_fingerprint: String,
    pub state: Option<IndexState>,
}

pub struct IndexReport {
    pub documents: usize,
    pub chunks: usize,
    pub collection: String,
}

pub fn rebuild(project: &Project) -> Result<IndexReport> {
    let docs = storage::collect_search_documents(project)?;
    let store = QdrantStore::for_project(project)?;

    if !store.is_available() {
        anyhow::bail!(
            "Qdrant is not reachable at {}; run 'yad infra up' first",
            project.config.qdrant.url
        );
    }

    store.recreate_collection()?;
    let mut embedding = EmbeddingService::new(&project.config.embedding.model)?;
    let mut total_chunks = 0usize;

    for batch in docs.chunks(32) {
        let mut pending = Vec::new();

        for doc in batch {
            for chunk in chunks_for_document(doc) {
                pending.push((doc, chunk));
            }
        }

        if pending.is_empty() {
            continue;
        }

        let texts = pending
            .iter()
            .map(|(_, chunk)| chunk.text.clone())
            .collect::<Vec<_>>();
        let vectors = embedding.embed_passages(&texts)?;
        let mut points = Vec::with_capacity(pending.len());

        for ((doc, chunk), vector) in pending.into_iter().zip(vectors) {
            total_chunks += 1;
            points.push(point_for(
                project,
                doc,
                chunk.index,
                chunk.label,
                chunk.text,
                vector,
            ));
        }

        store.upsert(&points)?;
    }

    let report = IndexReport {
        documents: docs.len(),
        chunks: total_chunks,
        collection: store.collection_name().to_string(),
    };
    write_state(project, report.documents, report.chunks, &report.collection)?;

    Ok(report)
}

pub fn index_document(project: &Project, doc: &SearchDocument) -> Result<usize> {
    let store = QdrantStore::for_project(project)?;
    if !store.is_available() {
        anyhow::bail!("Qdrant is not reachable at {}", project.config.qdrant.url);
    }

    store.ensure_collection()?;
    store.delete_source(&doc.id)?;

    let chunks = chunks_for_document(doc);
    let texts = chunks.iter().map(|c| c.text.clone()).collect::<Vec<_>>();
    let mut embedding = EmbeddingService::new(&project.config.embedding.model)?;
    let vectors = embedding.embed_passages(&texts)?;

    let points = chunks
        .into_iter()
        .zip(vectors)
        .map(|(chunk, vector)| {
            point_for(project, doc, chunk.index, chunk.label, chunk.text, vector)
        })
        .collect::<Vec<_>>();

    store.upsert(&points)?;
    Ok(points.len())
}

pub fn can_incremental_update(project: &Project) -> Result<bool> {
    let status = status(project)?;

    if status.has_state && !status.stale {
        let store = QdrantStore::for_project(project)?;
        return Ok(store.is_available() && store.collection_exists().unwrap_or(false));
    }

    if !status.has_state {
        return Ok(storage::collect_search_documents(project)?.is_empty());
    }

    Ok(false)
}

pub fn mark_current_if_unchanged(project: &Project, expected_fingerprint: &str) -> Result<bool> {
    let current = fingerprint(project)?;
    if current != expected_fingerprint {
        return Ok(false);
    }

    let docs = storage::collect_search_documents(project)?;
    let chunks = docs.iter().map(|doc| chunks_for_document(doc).len()).sum();
    let store = QdrantStore::for_project(project)?;

    if !store.is_available() || !store.collection_exists().unwrap_or(false) {
        return Ok(false);
    }

    write_state_with_fingerprint(
        project,
        expected_fingerprint,
        docs.len(),
        chunks,
        store.collection_name(),
    )?;
    Ok(true)
}

pub fn find_document(project: &Project, id: &str) -> Result<SearchDocument> {
    storage::collect_search_documents(project)?
        .into_iter()
        .find(|d| d.id.eq_ignore_ascii_case(id))
        .with_context(|| format!("document {} not found for indexing", id))
}

pub fn fingerprint(project: &Project) -> Result<String> {
    let mut docs = storage::collect_search_documents(project)?;
    docs.sort_by(|a, b| a.path.cmp(&b.path));

    let mut hasher = Sha256::new();
    hasher.update(b"yad-index-v3");
    hasher.update([0]);
    hasher.update(project.config.embedding.model.as_bytes());
    hasher.update([0]);
    hasher.update(project.config.embedding.dimensions.to_le_bytes());
    hasher.update([0]);
    hasher.update(embedding_revision(project).as_bytes());
    hasher.update([0xff]);

    for doc in docs {
        hasher.update(doc.id.as_bytes());
        hasher.update([0]);
        hasher.update(doc.source_type.as_bytes());
        hasher.update([0]);
        hasher.update(doc.kind.as_bytes());
        hasher.update([0]);
        hasher.update(doc.title.as_bytes());
        hasher.update([0]);
        hasher.update(doc.space.as_bytes());
        hasher.update([0]);
        hasher.update(doc.status.as_bytes());
        hasher.update([0]);
        hasher.update(doc.path.as_bytes());
        hasher.update([0]);
        hasher.update(doc.text.as_bytes());
        hasher.update([0xff]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

pub fn status(project: &Project) -> Result<IndexStatus> {
    let current_fingerprint = fingerprint(project)?;
    let path = state_path(project);
    if !path.exists() {
        return Ok(IndexStatus {
            has_state: false,
            stale: true,
            current_fingerprint,
            state: None,
        });
    }

    let raw = fs::read_to_string(&path)?;
    let state: IndexState = serde_json::from_str(&raw).context("invalid local index state")?;
    let stale = state.fingerprint != current_fingerprint;

    Ok(IndexStatus {
        has_state: true,
        stale,
        current_fingerprint,
        state: Some(state),
    })
}

fn write_state(project: &Project, documents: usize, chunks: usize, collection: &str) -> Result<()> {
    let fingerprint = fingerprint(project)?;
    write_state_with_fingerprint(project, &fingerprint, documents, chunks, collection)
}

fn write_state_with_fingerprint(
    project: &Project,
    fingerprint: &str,
    documents: usize,
    chunks: usize,
    collection: &str,
) -> Result<()> {
    fs::create_dir_all(project.runtime_dir())?;
    let state = IndexState {
        fingerprint: fingerprint.to_string(),
        indexed_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        documents,
        chunks,
        collection: collection.to_string(),
        embedding_model: project.config.embedding.model.clone(),
        embedding_dimensions: project.config.embedding.dimensions,
        embedding_revision: embedding_revision(project).to_string(),
    };
    atomic::write(state_path(project), serde_json::to_string_pretty(&state)?)?;
    Ok(())
}

fn state_path(project: &Project) -> std::path::PathBuf {
    project.runtime_dir().join("index-state.json")
}

#[derive(Debug)]
struct Chunk {
    index: usize,
    label: String,
    text: String,
}

fn chunks_for_document(doc: &SearchDocument) -> Vec<Chunk> {
    if doc.source_type == "memory" {
        return vec![Chunk {
            index: 0,
            label: doc.kind.clone(),
            text: format!("{}\n{}\n{}", doc.title, doc.space, doc.text.trim()),
        }];
    }

    let mut chunks = Vec::new();
    let normalized = doc.text.replace("\r\n", "\n");
    let mut current_label = "record".to_string();
    let mut current = String::new();

    for line in normalized.lines() {
        if let Some(label) = line.strip_prefix("## ") {
            if !current.trim().is_empty() {
                chunks.push(Chunk {
                    index: chunks.len(),
                    label: current_label.clone(),
                    text: format!("{}\n{}\n{}", doc.title, current_label, current.trim()),
                });
            }
            current_label = label.trim().to_string();
            current.clear();
            continue;
        }

        if line.starts_with("# ") {
            continue;
        }

        current.push_str(line);
        current.push('\n');
    }

    if !current.trim().is_empty() {
        chunks.push(Chunk {
            index: chunks.len(),
            label: current_label.clone(),
            text: format!("{}\n{}\n{}", doc.title, current_label, current.trim()),
        });
    }

    if chunks.is_empty() {
        chunks.push(Chunk {
            index: 0,
            label: "record".to_string(),
            text: format!("{}\n{}", doc.title, doc.text),
        });
    }

    chunks
}

fn point_for(
    project: &Project,
    doc: &SearchDocument,
    chunk_index: usize,
    chunk_label: String,
    text: String,
    vector: Vec<f32>,
) -> VectorPoint {
    let key = format!("{}:{}:{}", project.config.id, doc.id, chunk_index);
    let point_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, key.as_bytes()).to_string();

    VectorPoint {
        id: point_id,
        vector,
        payload: json!({
            "project_id": project.config.id,
            "source_id": doc.id,
            "source_type": doc.source_type,
            "kind": doc.kind,
            "title": doc.title,
            "space": doc.space,
            "space_ancestors": space_ancestors(&doc.space),
            "status": doc.status,
            "is_current": is_current_document(project, doc),
            "is_authoritative": is_authoritative_document(project, doc),
            "path": doc.path,
            "chunk_index": chunk_index,
            "chunk_label": chunk_label,
            "text": text,
        }),
    }
}

fn embedding_revision(project: &Project) -> &'static str {
    if project.config.embedding.model == model_cache::MODEL_NAME {
        model_cache::HF_REVISION
    } else {
        "provider-managed"
    }
}

fn space_ancestors(space: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();

    for segment in space.split('/') {
        if !current.is_empty() {
            current.push('/');
        }
        current.push_str(segment);
        result.push(current.clone());
    }

    result
}

fn is_current_document(project: &Project, doc: &SearchDocument) -> bool {
    if doc.source_type == "memory" {
        return !matches!(
            doc.status.to_ascii_lowercase().as_str(),
            "archived" | "superseded"
        );
    }

    if doc.source_type == "record" {
        return schema::resolve(&project.yad_dir, &doc.kind)
            .map(|definition| !definition.is_historical_status(&doc.status))
            .unwrap_or(true);
    }

    true
}

fn is_authoritative_document(project: &Project, doc: &SearchDocument) -> bool {
    if doc.source_type == "memory" {
        return doc.kind == "decision" && doc.status == "active";
    }

    doc.source_type == "record"
        && schema::resolve(&project.yad_dir, &doc.kind)
            .map(|definition| definition.is_authoritative_status(&doc.status))
            .unwrap_or(false)
}