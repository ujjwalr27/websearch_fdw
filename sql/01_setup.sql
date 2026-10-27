-- websearch_fdw: one-time setup.
-- Run after the .wasm package is reachable (scripts/dev.sh copies it into the local Supabase db container).

-- 1. Enable Wrappers and register the Wasm FDW handler
create extension if not exists wrappers with schema extensions;

create foreign data wrapper wasm_wrapper
  handler wasm_fdw_handler
  validator wasm_fdw_validator;

-- 2. Store the search provider's API key in Vault (replace the placeholder first)
select vault.create_secret('<YOUR_API_KEY>', 'websearch_api_key', 'API key for websearch_fdw');

-- 3. Create the server, pointing at the Vault secret by id
do $$
declare
  key_id text;
begin
  select id into key_id from vault.secrets where name = 'websearch_api_key';

  execute format(
    $sql$
    create server websearch_server
      foreign data wrapper wasm_wrapper
      options (
        fdw_package_url 'file:///websearch_fdw.wasm',
        fdw_package_name 'websearch:fdw',
        fdw_package_version '0.1.0',
        provider 'serpapi',
        api_key_id %L
      )
    $sql$,
    key_id
  );
end $$;

-- Offline mode for demos (no network, no API quota):
--   alter server websearch_server options (set provider 'fixture');
-- Back to live results:
--   alter server websearch_server options (set provider 'serpapi');
