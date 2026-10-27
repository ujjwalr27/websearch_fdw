//! Search providers. The FDW only talks to the `SearchProvider` trait, so a
//! backend can be added or swapped without touching the Postgres-facing code.

mod fixture;
mod serpapi;

use serde_json::{Map, Value as JsonValue};

pub use fixture::FixtureProvider;
pub use serpapi::SerpApiProvider;

/// What kind of results a foreign table returns, set by its `kind` option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKind {
    Web,
    Shopping,
    News,
}

impl SearchKind {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "web" => Ok(Self::Web),
            "shopping" => Ok(Self::Shopping),
            "news" => Ok(Self::News),
            other => Err(format!(
                "unknown kind '{other}', expected one of: web, shopping, news"
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub kind: SearchKind,
    pub query: String,
    pub location: Option<String>,
    pub language: Option<String>,
}

/// One result, keyed by normalized column name (title, link, price, ...).
pub type Record = Map<String, JsonValue>;

pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// Performs an HTTP GET. Injected by the FDW so providers stay free of host bindings.
pub type Fetch<'a> = &'a dyn Fn(&str) -> Result<HttpResponse, String>;

pub trait SearchProvider {
    fn search(&self, req: &SearchRequest, fetch: Fetch<'_>) -> Result<Vec<Record>, String>;
}

pub fn from_name(name: &str, api_key: Option<String>) -> Result<Box<dyn SearchProvider>, String> {
    match name {
        "serpapi" => {
            let key = api_key.ok_or("provider 'serpapi' needs an api_key_id or api_key server option")?;
            Ok(Box::new(SerpApiProvider::new(key)))
        }
        "fixture" => Ok(Box::new(FixtureProvider)),
        other => Err(format!(
            "unknown provider '{other}', expected one of: serpapi, fixture"
        )),
    }
}
