//! SerpApi adapter: maps SerpApi's Google engines onto the normalized record shape.

use serde_json::Value as JsonValue;

use super::{Fetch, Record, SearchKind, SearchProvider, SearchRequest};

const BASE_URL: &str = "https://serpapi.com/search.json";

pub struct SerpApiProvider {
    api_key: String,
}

impl SerpApiProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    fn url(&self, req: &SearchRequest) -> String {
        let mut params: Vec<(&str, &str)> = vec![
            ("q", req.query.as_str()),
            ("api_key", self.api_key.as_str()),
        ];
        match req.kind {
            SearchKind::Web => params.push(("engine", "google")),
            SearchKind::Shopping => params.push(("engine", "google_shopping")),
            SearchKind::News => {
                params.push(("engine", "google"));
                params.push(("tbm", "nws"));
            }
        }
        if let Some(location) = &req.location {
            params.push(("location", location));
        }
        if let Some(language) = &req.language {
            params.push(("hl", language));
        }

        let query: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{k}={}", encode(v)))
            .collect();
        format!("{BASE_URL}?{}", query.join("&"))
    }
}

impl SearchProvider for SerpApiProvider {
    fn search(&self, req: &SearchRequest, fetch: Fetch<'_>) -> Result<Vec<Record>, String> {
        // never let the key leak into a Postgres error message
        let resp = fetch(&self.url(req)).map_err(|e| e.replace(&self.api_key, "***"))?;
        parse(req.kind, resp.status, &resp.body)
    }
}

fn parse(kind: SearchKind, status: u16, body: &str) -> Result<Vec<Record>, String> {
    let resp: JsonValue = serde_json::from_str(body)
        .map_err(|_| format!("search provider returned HTTP {status} with a non-JSON body"))?;

    if let Some(err) = resp.get("error").and_then(JsonValue::as_str) {
        // an empty result set comes back as an "error", but it is not a failure
        if err.contains("hasn't returned any results") {
            return Ok(Vec::new());
        }
        return Err(format!("search provider error: {err}"));
    }
    if status >= 400 {
        return Err(format!("search provider returned HTTP {status}"));
    }

    let key = match kind {
        SearchKind::Web => "organic_results",
        SearchKind::Shopping => "shopping_results",
        SearchKind::News => "news_results",
    };
    let items = resp.get(key).and_then(JsonValue::as_array).cloned().unwrap_or_default();
    Ok(items.iter().map(|item| normalize(kind, item)).collect())
}

fn normalize(kind: SearchKind, item: &JsonValue) -> Record {
    // first non-null field among the candidates
    let pick = |keys: &[&str]| {
        keys.iter()
            .find_map(|k| item.get(*k).filter(|v| !v.is_null()).cloned())
            .unwrap_or(JsonValue::Null)
    };

    let mut rec = Record::new();
    rec.insert("position".into(), pick(&["position"]));
    rec.insert("title".into(), pick(&["title"]));
    rec.insert("link".into(), pick(&["link", "product_link"]));
    match kind {
        SearchKind::Web => {
            rec.insert("snippet".into(), pick(&["snippet"]));
            rec.insert("source".into(), pick(&["source", "displayed_link"]));
        }
        SearchKind::Shopping => {
            rec.insert("price".into(), pick(&["extracted_price"]));
            rec.insert("price_text".into(), pick(&["price"]));
            rec.insert("source".into(), pick(&["source"]));
            rec.insert("rating".into(), pick(&["rating"]));
            rec.insert("reviews".into(), pick(&["reviews"]));
            rec.insert("thumbnail".into(), pick(&["thumbnail"]));
        }
        SearchKind::News => {
            let source = pick(&["source"]);
            let source = source.get("name").cloned().unwrap_or(source);
            rec.insert("snippet".into(), pick(&["snippet"]));
            rec.insert("source".into(), source);
            rec.insert("published".into(), pick(&["date"]));
        }
    }
    rec.insert("raw".into(), item.clone());
    rec
}

// percent-encode a query-string value (RFC 3986 unreserved characters pass through)
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
