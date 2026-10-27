# websearch_fdw

**Query the live web from Postgres, as if it were a table.**

> TODO: one-sentence hook + demo GIF / screenshot here.

```sql
select title, price, source
from web.shopping
where query = 'Sony WH-1000XM5'
order by price
limit 5;
```

## Why

Apps on PostgreSQL or Supabase usually need a separate backend service just to pull search results in. `websearch_fdw` removes that layer: search runs inside the database and results come back as ordinary rows, so you can

- **join** them with your own tables,
- **store** them for later analysis,
- **collect** them on a schedule with `pg_cron`.

> TODO: 2–3 sentences on the problem from your own experience.

## How it works

```
 SQL query ──▶ Postgres ──▶ websearch_fdw (Wasm) ──▶ provider adapter ──▶ web search API
                  ▲                                                             │
                  └────────────────── rows ◀── normalize JSON ◀─────────────────┘
```

- Built as a **Wasm foreign data wrapper** on Supabase's [Wrappers](https://github.com/supabase/wrappers) framework, so it loads into hosted Supabase without a custom Postgres build.
- `WHERE query = '...'` (plus optional `location` and `language`) is **pushed down** to the search provider.
- Providers sit behind one small trait ([src/provider/mod.rs](src/provider/mod.rs)), so backends are swappable. It ships with:
  - `serpapi`: live results (Google web, shopping and news via SerpApi)
  - `fixture`: bundled offline data for demos and tests
- API keys live in **Supabase Vault**, never in plain SQL.

## Quickstart (local)

**Prerequisites:** Rust, Docker, [Supabase CLI](https://supabase.com/docs/guides/local-development) (`npx supabase`).

The repo pins Rust 1.81.0 and its wasm targets in [rust-toolchain.toml](rust-toolchain.toml); rustup installs them on first build.

```bash
cargo install cargo-component --version 0.13.2 --locked

npx supabase init      # once
npx supabase start
./scripts/dev.sh       # build + copy websearch_fdw.wasm into the db container
```

Then run, in order, in the SQL editor (http://localhost:54323):

1. [sql/01_setup.sql](sql/01_setup.sql): extension, Vault secret, server (put your API key in first)
2. [sql/02_tables.sql](sql/02_tables.sql): the `web.*` foreign tables
3. [sql/03_examples.sql](sql/03_examples.sql): try it

No API key? Run `alter server websearch_server options (set provider 'fixture');` to use offline data.

## Tables

| Table | Filters | Columns |
|---|---|---|
| `web.search` | `query` (required), `location`, `language` | `position, title, link, snippet, source, raw` |
| `web.shopping` | same | `position, title, price, price_text, source, rating, reviews, link, thumbnail, raw` |
| `web.news` | same | `position, title, link, snippet, source, published, raw` |

`raw` (jsonb) holds the provider's full result for anything not exposed as a column.

To run one search per row of another table, wrap the table in a function that passes the term as a literal (a plain join hides it from the wrapper). See `market_prices()` in the demo:

```sql
select p.name, min(m.price)
from products p
cross join lateral market_prices(p.search_term) m
group by p.name;
```

## Demo: price tracker

[demo/price_tracker.sql](demo/price_tracker.sql) builds a tiny store catalog, pulls market prices into `price_snapshots` with `refresh_prices()`, and shows where each product stands in `price_position`. One `cron.schedule` line makes it run hourly.

Live shopping results are noisy: look-alike models, spare parts that name the product, resellers at triple the price. Because the results are rows, the demo cleans them with plain SQL:

1. **Model match**: an offer's title must contain the product's model code (`matches_model()`).
2. **Price band**: only offers between 0.5× and 2× your own price count.
3. **Median of the top 10** of what's left, instead of the cheapest offer.
4. **Confidence**: `high` (5+ offers), `medium` (3–4) or `low`, so a thin result isn't trusted blindly.

> TODO: screenshot of `price_position` / price history chart.

## Project structure

```
├── src/
│   ├── lib.rs               # FDW routines: options, qual pushdown, JSON → Postgres types
│   └── provider/
│       ├── mod.rs           # SearchProvider trait + normalized record shape
│       ├── serpapi.rs       # live search adapter
│       └── fixture.rs       # offline adapter
├── fixtures/                # offline results, keyed by lowercase query
├── sql/                     # setup, tables, examples
├── demo/price_tracker.sql   # end-to-end demo
├── scripts/dev.sh           # local build + load
├── wit/, supabase-wrappers-wit/   # Wasm interface definitions (from the Supabase template)
└── .github/workflows/release.yml  # tag v* → GitHub release with the .wasm
```

## Deploying to hosted Supabase

1. Push a tag (`git tag v0.1.0 && git push --tags`) from a **public** repo. The release workflow publishes `websearch_fdw.wasm`, plus a `README.txt` with its URL and checksum.
2. In your Supabase project, add your API key on the Vault page as `websearch_api_key`.
3. Fill the URL and checksum into [sql/01_setup_hosted.sql](sql/01_setup_hosted.sql) and run it, then [sql/02_tables.sql](sql/02_tables.sql).

## Roadmap

- [ ] `LIMIT` pushdown and pagination
- [ ] Response cache table with TTL
- [ ] Second live provider adapter
- [ ] Images / local results tables

## Team

> TODO: names, roles, links. Hackathon: TODO.

## License

Apache-2.0. Wasm FDW scaffolding adapted from [supabase-community/postgres-wasm-fdw](https://github.com/supabase-community/postgres-wasm-fdw).
