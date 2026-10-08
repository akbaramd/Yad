use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub embedding: EmbeddingConfig,
    #[serde(default)]
    pub search: SearchConfig,
    pub qdrant: QdrantConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    pub model: String,
    pub dimensions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    pub min_semantic_score: f32,
    #[serde(default = "default_semantic_candidate_limit")]
    pub semantic_candidate_limit: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            min_semantic_score: 0.80,
            semantic_candidate_limit: default_semantic_candidate_limit(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantConfig {
    pub url: String,

    // Kept only for reading early MVP project files. New projects do not persist
    // a collection name because vector indexes are workspace-local, not shared
    // project truth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
}

impl ProjectConfig {
    pub fn new(id: String, name: String) -> Self {
        Self {
            version: 1,
            id,
            name,
            embedding: EmbeddingConfig {
                model: "multilingual-e5-small-int8".to_string(),
                dimensions: 384,
            },
            search: SearchConfig::default(),
            qdrant: QdrantConfig {
                url: "http://127.0.0.1:6333".to_string(),
                collection: None,
            },
        }
    }
}

fn default_semantic_candidate_limit() -> usize {
    512
}
