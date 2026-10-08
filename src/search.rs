use std::collections::{HashMap, HashSet};

use anyhow::Result;
use serde_json::{Value, json};

use crate::{
    embedding::EmbeddingService,
    index,
    model::{SearchDocument, SearchResult},
    project::Project,
    qdrant::QdrantStore,
    storage,
};

#[derive(Debug)]
pub struct SearchOutcome {
    pub results: Vec<SearchResult>,
    pub semantic_used: bool,
    pub semantic_warning: Option<String>,
}

pub fn search(
    project: &Project,
    query: &str,
    limit: usize,
    space: Option<&str>,
    kind: Option<&str>,
    include_history: bool,
) -> Result<SearchOutcome> {
    if query.trim().is_empty() {
        anyhow::bail!("search query cannot be empty");
    }

    let docs = storage::collect_search_documents(project)?;
    let filtered = docs
        .iter()
        .filter(|d| matches_filter(d, space, kind, include_history))
        .cloned()
        .collect::<Vec<_>>();

    let candidate_limit = project
        .config
        .search
        .semantic_candidate_limit
        .max(limit.saturating_mul(20))
        .clamp(100, 4096);
    let lexical = lexical_rank(&filtered, query, candidate_limit);

    let store = QdrantStore::for_project(project)?;
    let mut semantic_used = false;
    let mut semantic_warning = None;
    let mut semantic = Vec::new();

    let index_status = index::status(project)?;
    if index_status.stale {
        semantic_warning =
            Some("semantic index is stale; run 'yad sync' or 'yad index rebuild'".to_string());
    } else if store.is_available() {
        match semantic_rank(
            project,
            &store,
            query,
            candidate_limit,
            space,
            kind,
            include_history,
        ) {
            Ok(results) => {
                semantic_used = true;
                semantic = results;
            }
            Err(err) => semantic_warning = Some(err.to_string()),
        }
    } else {
        semantic_warning = Some(format!(
            "Qdrant is unavailable at {}; using lexical search only",
            project.config.qdrant.url
        ));
    }

    let results = fuse(lexical, semantic, limit, include_history);
    Ok(SearchOutcome {
        results,
        semantic_used,
        semantic_warning,
    })
}

fn semantic_rank(
    project: &Project,
    store: &QdrantStore,
    query: &str,
    limit: usize,
    space: Option<&str>,
    kind: Option<&str>,
    include_history: bool,
) -> Result<Vec<SearchResult>> {
    if !store.collection_exists()? {
        anyhow::bail!("Qdrant collection does not exist; run 'yad index rebuild'");
    }

    let mut embedding = EmbeddingService::new(&project.config.embedding.model)?;
    let vector = embedding.embed_query(query)?;
    let filter = qdrant_filter(space, kind, include_history);
    let mut points = store.search(&vector, limit, filter.as_ref())?;

    if kind.is_none() {
        let authority_filter = qdrant_authority_filter(space, include_history);
        let mut authority_points = store.search(&vector, 64, Some(&authority_filter))?;
        points.append(&mut authority_points);
    }

    let mut best: HashMap<String, SearchResult> = HashMap::new();

    for point in points {
        if point.score < project.config.search.min_semantic_score {
            continue;
        }

        let payload = &point.payload;
        let get = |key: &str| {
            payload
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };

        let result = SearchResult {
            id: get("source_id"),
            source_type: get("source_type"),
            kind: get("kind"),
            title: get("title"),
            space: get("space"),
            status: get("status"),
            path: get("path"),
            score: point.score,
            semantic_score: Some(point.score),
            lexical_score: None,
            snippet: compact_snippet(&get("text"), 320),
        };

        if !matches_result_filter(&result, space, kind, include_history) {
            continue;
        }

        match best.get(&result.id) {
            Some(existing) if existing.score >= result.score => {}
            _ => {
                best.insert(result.id.clone(), result);
            }
        }
    }

    let mut values = best.into_values().collect::<Vec<_>>();
    values.sort_by(|a, b| b.score.total_cmp(&a.score));
    Ok(values)
}

fn lexical_rank(docs: &[SearchDocument], query: &str, limit: usize) -> Vec<SearchResult> {
    let query_lower = normalize_text(query);
    let terms = tokenize(query);
    let mut scored = Vec::new();

    for doc in docs {
        let title_lower = normalize_text(&doc.title);
        let text_lower = normalize_text(&doc.text);
        let title_terms = tokenize(&doc.title).into_iter().collect::<HashSet<_>>();
        let text_terms = tokenize(&doc.text).into_iter().collect::<HashSet<_>>();
        let space_terms = tokenize(&doc.space).into_iter().collect::<HashSet<_>>();
        let mut score = 0.0f32;

        if title_lower.contains(&query_lower) {
            score += 12.0;
        }
        if text_lower.contains(&query_lower) {
            score += 8.0;
        }

        for term in &terms {
            if title_terms.contains(term) {
                score += 4.0;
            }
            if space_terms.contains(term) {
                score += 2.0;
            }
            if text_terms.contains(term) {
                score += 1.0;
            }
        }

        score *= status_weight(&doc.status);

        if score > 0.0 {
            scored.push(SearchResult {
                id: doc.id.clone(),
                source_type: doc.source_type.clone(),
                kind: doc.kind.clone(),
                title: doc.title.clone(),
                space: doc.space.clone(),
                status: doc.status.clone(),
                path: doc.path.clone(),
                score,
                semantic_score: None,
                lexical_score: Some(score),
                snippet: compact_snippet(&doc.text, 320),
            });
        }
    }

    scored.sort_by(|a, b| b.score.total_cmp(&a.score));
    scored.truncate(limit);
    scored
}

fn fuse(
    lexical: Vec<SearchResult>,
    semantic: Vec<SearchResult>,
    limit: usize,
    include_history: bool,
) -> Vec<SearchResult> {
    let max_lexical = lexical
        .iter()
        .filter_map(|item| item.lexical_score)
        .fold(0.0f32, f32::max);

    let mut merged: HashMap<String, SearchResult> = HashMap::new();

    for item in lexical {
        merged.insert(item.id.clone(), item);
    }

    for item in semantic {
        merged
            .entry(item.id.clone())
            .and_modify(|existing| {
                existing.semantic_score = item.semantic_score;
                if item.snippet.len() > existing.snippet.len() {
                    existing.snippet = item.snippet.clone();
                }
            })
            .or_insert(item);
    }

    let mut results = merged
        .into_values()
        .map(|mut item| {
            let lexical_norm = item
                .lexical_score
                .map(|score| {
                    if max_lexical > 0.0 {
                        (score / max_lexical).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                })
                .unwrap_or(0.0);

            let base = match item.semantic_score {
                Some(semantic) => semantic * 0.80 + lexical_norm * 0.20,
                None => lexical_norm * 0.55,
            };

            let lifecycle_weight = if include_history {
                1.0
            } else {
                status_weight(&item.status)
            };
            item.score = base * lifecycle_weight + authority_bonus(&item);
            item
        })
        .collect::<Vec<_>>();

    results.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| {
                b.semantic_score
                    .unwrap_or_default()
                    .total_cmp(&a.semantic_score.unwrap_or_default())
            })
            .then_with(|| {
                b.lexical_score
                    .unwrap_or_default()
                    .total_cmp(&a.lexical_score.unwrap_or_default())
            })
            .then_with(|| a.id.cmp(&b.id))
    });
    results.truncate(limit);
    results
}

fn matches_filter(
    doc: &SearchDocument,
    space: Option<&str>,
    kind: Option<&str>,
    include_history: bool,
) -> bool {
    if !include_history && !is_current_status(&doc.status) {
        return false;
    }

    if let Some(space) = space
        && !doc.space.eq_ignore_ascii_case(space)
        && !doc
            .space
            .to_lowercase()
            .starts_with(&format!("{}/", space.to_lowercase()))
    {
        return false;
    }

    if let Some(kind) = kind
        && !doc.kind.eq_ignore_ascii_case(kind)
    {
        return false;
    }
    true
}

fn matches_result_filter(
    result: &SearchResult,
    space: Option<&str>,
    kind: Option<&str>,
    include_history: bool,
) -> bool {
    if !include_history && !is_current_status(&result.status) {
        return false;
    }

    if let Some(space) = space
        && !result.space.eq_ignore_ascii_case(space)
        && !result
            .space
            .to_lowercase()
            .starts_with(&format!("{}/", space.to_lowercase()))
    {
        return false;
    }

    if let Some(kind) = kind
        && !result.kind.eq_ignore_ascii_case(kind)
    {
        return false;
    }
    true
}

fn tokenize(value: &str) -> Vec<String> {
    normalize_text(value)
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|value| value.chars().count() >= 2)
        .filter(|value| !is_stopword(value))
        .map(ToString::to_string)
        .collect()
}

fn normalize_text(value: &str) -> String {
    value.to_lowercase().replace('ي', "ی").replace('ك', "ک")
}

fn is_stopword(value: &str) -> bool {
    matches!(
        value,
        // English
        "a" | "an" | "the" | "is" | "are" | "was" | "were" | "be" | "been"
            | "being" | "do" | "does" | "did" | "how" | "what" | "why" | "when"
            | "where" | "which" | "who" | "whom" | "whose" | "with" | "for"
            | "from" | "to" | "of" | "in" | "on" | "at" | "by" | "and" | "or"
            | "this" | "that" | "these" | "those" | "it" | "its" | "i" | "we"
            | "you" | "they" | "my" | "our" | "your" | "their" | "can" | "could"
            | "would" | "should"
            // Persian
            | "این" | "آن" | "است" | "هست" | "بود" | "باشد" | "برای" | "از"
            | "به" | "در" | "با" | "را" | "که" | "چه" | "چرا" | "چطور"
            | "چگونه" | "آیا" | "یک" | "و" | "یا" | "تا" | "روی" | "بعد"
            | "قبل" | "اگر" | "وقتی" | "میشه" | "شود" | "شده" | "کردن"
    )
}

fn compact_snippet(value: &str, max_chars: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        normalized
    } else {
        normalized.chars().take(max_chars).collect::<String>() + "…"
    }
}

fn status_weight(status: &str) -> f32 {
    match status {
        "archived" => 0.35,
        "superseded" | "deprecated" => 0.55,
        _ => 1.0,
    }
}

fn is_current_status(status: &str) -> bool {
    !matches!(
        status.to_ascii_lowercase().as_str(),
        "archived" | "superseded" | "deprecated" | "rejected"
    )
}

fn authority_bonus(result: &SearchResult) -> f32 {
    match (
        result.source_type.as_str(),
        result.kind.as_str(),
        result.status.as_str(),
    ) {
        ("record", "adr", "accepted") => 0.015,
        ("memory", "decision", "active") => 0.005,
        ("record", "adr", "proposed") => 0.002,
        _ => 0.0,
    }
}

fn qdrant_filter(space: Option<&str>, kind: Option<&str>, include_history: bool) -> Option<Value> {
    let mut must = Vec::new();

    if let Some(space) = space {
        must.push(json!({
            "key": "space_ancestors",
            "match": { "value": space }
        }));
    }

    if let Some(kind) = kind {
        must.push(json!({
            "key": "kind",
            "match": { "value": kind }
        }));
    }

    if !include_history {
        must.push(json!({
            "key": "is_current",
            "match": { "value": true }
        }));
    }

    if must.is_empty() {
        None
    } else {
        Some(json!({ "must": must }))
    }
}

fn qdrant_authority_filter(space: Option<&str>, include_history: bool) -> Value {
    let mut must = vec![json!({
        "key": "kind",
        "match": { "value": "adr" }
    })];

    if let Some(space) = space {
        must.push(json!({
            "key": "space_ancestors",
            "match": { "value": space }
        }));
    }

    if !include_history {
        must.push(json!({
            "key": "is_current",
            "match": { "value": true }
        }));
    }

    json!({ "must": must })
}
