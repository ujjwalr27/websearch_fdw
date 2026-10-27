#[allow(warnings)]
mod bindings;
mod provider;

use serde_json::Value as JsonValue;

use bindings::{
    exports::supabase::wrappers::routines::Guest,
    supabase::wrappers::{
        http,
        types::{Cell, Context, FdwError, FdwResult, OptionsType, Row, TypeOid, Value},
        utils,
    },
};
use provider::{HttpResponse, Record, SearchKind, SearchProvider, SearchRequest};

#[derive(Default)]
struct WebSearchFdw {
    provider: Option<Box<dyn SearchProvider>>,
    rows: Vec<Record>,
    row_idx: usize,
}

// pointer for the static FDW instance
static mut INSTANCE: *mut WebSearchFdw = std::ptr::null_mut::<WebSearchFdw>();

impl WebSearchFdw {
    // initialise FDW instance
    fn init_instance() {
        let instance = Self::default();
        unsafe {
            INSTANCE = Box::leak(Box::new(instance));
        }
    }

    fn this_mut() -> &'static mut Self {
        unsafe { &mut (*INSTANCE) }
    }

    // build a search request from the table options and WHERE clause, then run it
    fn fetch(&mut self, ctx: &Context) -> FdwResult {
        let opts = ctx.get_options(OptionsType::Table);
        let kind = SearchKind::parse(&opts.require_or("kind", "web"))?;

        let (mut query, mut location, mut language) = (None, None, None);
        for qual in ctx.get_quals() {
            if qual.operator() != "=" || qual.use_or() {
                continue;
            }
            let Value::Cell(Cell::String(value)) = qual.value() else {
                continue;
            };
            match qual.field().as_str() {
                "query" => query = Some(value),
                "location" => location = Some(value),
                "language" => language = Some(value),
                _ => {}
            }
        }
        let query = query.ok_or("a search needs a query, e.g. WHERE query = 'wireless headphones'")?;
        let req = SearchRequest {
            kind,
            query,
            location,
            language,
        };

        let fetch = |url: &str| -> Result<HttpResponse, String> {
            let resp = http::get(&http::Request {
                method: http::Method::Get,
                url: url.to_owned(),
                headers: vec![("user-agent".to_owned(), "websearch_fdw".to_owned())],
                body: String::default(),
            })?;
            Ok(HttpResponse {
                status: resp.status_code,
                body: resp.body,
            })
        };
        let provider = self.provider.as_ref().ok_or("search provider is not initialised")?;
        let mut rows = provider.search(&req, &fetch)?;

        // echo the pushed-down filters back, so Postgres's own recheck of the WHERE clause keeps every row
        for (i, row) in rows.iter_mut().enumerate() {
            row.insert("query".into(), req.query.as_str().into());
            if let Some(location) = &req.location {
                row.insert("location".into(), location.as_str().into());
            }
            if let Some(language) = &req.language {
                row.insert("language".into(), language.as_str().into());
            }
            if row.get("position").map_or(true, JsonValue::is_null) {
                row.insert("position".into(), (i + 1).into());
            }
        }

        self.rows = rows;
        self.row_idx = 0;
        Ok(())
    }
}

// convert a JSON value into the Postgres type its column was declared with
fn to_cell(v: &JsonValue, type_oid: TypeOid) -> Result<Option<Cell>, FdwError> {
    if v.is_null() {
        return Ok(None);
    }
    let num = || {
        v.as_f64()
            .or_else(|| v.as_str().and_then(|s| s.replace(',', "").trim().parse().ok()))
    };
    let cell = match type_oid {
        TypeOid::Bool => v.as_bool().map(Cell::Bool),
        TypeOid::I32 => num().map(|n| Cell::I32(n as i32)),
        TypeOid::I64 => num().map(|n| Cell::I64(n as i64)),
        TypeOid::F64 => num().map(Cell::F64),
        TypeOid::Numeric => num().map(Cell::Numeric),
        TypeOid::String => Some(Cell::String(
            v.as_str().map_or_else(|| v.to_string(), str::to_owned),
        )),
        TypeOid::Json => Some(Cell::Json(v.to_string())),
        _ => {
            return Err(
                "supported column types are: bool, int, bigint, float8, numeric, text, jsonb"
                    .to_owned(),
            )
        }
    };
    Ok(cell)
}

impl Guest for WebSearchFdw {
    fn host_version_requirement() -> String {
        // semver expression for Wasm FDW host version requirement
        // ref: https://docs.rs/semver/latest/semver/enum.Op.html
        "^0.1.0".to_string()
    }

    fn init(ctx: &Context) -> FdwResult {
        Self::init_instance();
        let this = Self::this_mut();

        let opts = ctx.get_options(OptionsType::Server);
        let provider_name = opts.require_or("provider", "serpapi");
        // prefer a Vault secret; a plain api_key option is only for local development
        let api_key = match opts.get("api_key_id") {
            Some(id) => Some(
                utils::get_vault_secret(&id).ok_or(format!("vault secret '{id}' not found"))?,
            ),
            None => opts.get("api_key"),
        };
        this.provider = Some(provider::from_name(&provider_name, api_key)?);

        Ok(())
    }

    fn begin_scan(ctx: &Context) -> FdwResult {
        Self::this_mut().fetch(ctx)
    }

    fn iter_scan(ctx: &Context, row: &Row) -> Result<Option<u32>, FdwError> {
        let this = Self::this_mut();

        let Some(src) = this.rows.get(this.row_idx) else {
            return Ok(None);
        };
        for col in ctx.get_columns() {
            let cell = match src.get(&col.name()) {
                Some(v) => to_cell(v, col.type_oid())?,
                None => None,
            };
            row.push(cell.as_ref());
        }

        this.row_idx += 1;

        Ok(Some(0))
    }

    // called when Postgres restarts the scan, e.g. on the inner side of a nested loop
    fn re_scan(ctx: &Context) -> FdwResult {
        Self::this_mut().fetch(ctx)
    }

    fn end_scan(_ctx: &Context) -> FdwResult {
        let this = Self::this_mut();
        this.rows.clear();
        Ok(())
    }

    fn begin_modify(_ctx: &Context) -> FdwResult {
        Err("search results are read-only".to_owned())
    }

    fn insert(_ctx: &Context, _row: &Row) -> FdwResult {
        Ok(())
    }

    fn update(_ctx: &Context, _rowid: Cell, _row: &Row) -> FdwResult {
        Ok(())
    }

    fn delete(_ctx: &Context, _rowid: Cell) -> FdwResult {
        Ok(())
    }

    fn end_modify(_ctx: &Context) -> FdwResult {
        Ok(())
    }
}

bindings::export!(WebSearchFdw with_types_in bindings);
