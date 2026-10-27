-- websearch_fdw: foreign tables.
-- Every query must filter on `query`; `location` and `language` are optional filters
-- that are also sent to the search provider. `raw` holds the provider's full result.

create schema if not exists web;

create foreign table web.search (
  query     text,
  position  int,
  title     text,
  link      text,
  snippet   text,
  source    text,
  location  text,
  language  text,
  raw       jsonb
)
  server websearch_server
  options (kind 'web');

create foreign table web.shopping (
  query       text,
  position    int,
  title       text,
  price       numeric,
  price_text  text,
  source      text,
  rating      numeric,
  reviews     int,
  link        text,
  thumbnail   text,
  location    text,
  language    text,
  raw         jsonb
)
  server websearch_server
  options (kind 'shopping');

create foreign table web.news (
  query      text,
  position   int,
  title      text,
  link       text,
  snippet    text,
  source     text,
  published  text,
  location   text,
  language   text,
  raw        jsonb
)
  server websearch_server
  options (kind 'news');
