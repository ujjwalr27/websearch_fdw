-- websearch_fdw: one-time setup on a hosted Supabase project (use sql/01_setup.sql for local).
-- Before running:
--   1. Publish a release by pushing a v* tag; its README.txt has the .wasm URL and checksum.
--   2. Add your search API key on the dashboard's Vault page, named websearch_api_key.
--      Adding it there keeps the key out of SQL editor history.
--   3. Replace the two <...> placeholders below.

create extension if not exists wrappers with schema extensions;

create foreign data wrapper wasm_wrapper
  handler wasm_fdw_handler
  validator wasm_fdw_validator;

do $$
declare
  key_id text;
begin
  select id into key_id from vault.secrets where name = 'websearch_api_key';
  if key_id is null then
    raise exception 'add a Vault secret named websearch_api_key first';
  end if;

  execute format(
    $sql$
    create server websearch_server
      foreign data wrapper wasm_wrapper
      options (
        fdw_package_url '<RELEASE_WASM_URL>',
        fdw_package_name 'websearch:fdw',
        fdw_package_version '0.1.0',
        fdw_package_checksum '<SHA256_CHECKSUM>',
        provider 'serpapi',
        api_key_id %L
      )
    $sql$,
    key_id
  );
end $$;

-- Then run sql/02_tables.sql, and optionally demo/price_tracker.sql.
