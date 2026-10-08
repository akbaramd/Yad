use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMeta {
    pub yad: u32,
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub kind: String,
    pub space: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub status: String,
    pub importance: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub created: String,
    pub updated: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
}

/// Generic metadata shared by every schema-backed formal record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordMeta {
    pub yad: u32,
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub status: String,
    pub space: String,
    pub created: String,
    pub updated: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supersedes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub superseded_by: Vec<String>,
}

/// Kept as a compatibility type for the dedicated `yad adr` shorthand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdrMeta {
    pub yad: u32,
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub status: String,
    pub space: String,
    pub created: String,
    pub updated: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supersedes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub superseded_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub id: String,
    pub source_type: String,
    pub kind: String,
    pub title: String,
    pub space: String,
    pub status: String,
    pub path: String,
    pub score: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lexical_score: Option<f32>,
    pub snippet: String,
}

#[derive(Debug, Clone)]
pub struct SearchDocument {
    pub id: String,
    pub source_type: String,
    pub kind: String,
    pub title: String,
    pub space: String,
    pub status: String,
    pub path: String,
    pub text: String,
}
