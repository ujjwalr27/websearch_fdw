//! Offline provider backed by bundled JSON, for demos without network access or API quota.
//!
//! Each fixture file maps a lowercase query to its results; `"*"` is the fallback.

use serde_json::{Map, Value as JsonValue};

use super::{Fetch, Record, SearchKind, SearchProvider, SearchRequest};

pub struct FixtureProvider;

impl SearchProvider for FixtureProvider {
    fn search(&self, req: &SearchRequest, _fetch: Fetch<'_>) -> Result<Vec<Record>, String> {
        let raw = match req.kind {
            SearchKind::Web => include_str!("../../fixtures/web.json"),
            SearchKind::Shopping => include_str!("../../fixtures/shopping.json"),
            SearchKind::News => include_str!("../../fixtures/news.json"),
        };
        let by_query: Map<String, JsonValue> =
            serde_json::from_str(raw).map_err(|e| format!("invalid fixture file: {e}"))?;

        let key = req.query.trim().to_lowercase();
        let results = by_query.get(&key).or_else(|| by_query.get("*"));
        Ok(results
            .and_then(JsonValue::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_object().cloned())
                    .map(|mut rec| {
                        // mirror live providers, which keep the full result in `raw`
                        rec.insert("raw".into(), JsonValue::Object(rec.clone()));
                        rec
                    })
                    .collect()
            })
            .unwrap_or_default())
    }
}
