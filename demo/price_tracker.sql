-- Demo: a price tracker that compares your catalog with live market prices.
-- Requires sql/01_setup.sql and sql/02_tables.sql. The sample search terms match fixtures/shopping.json,
-- so the whole demo also runs offline with provider 'fixture'.

-- Your application's own data. Search results are loose (a search for one model returns its
-- neighbours too), so each product also has a model code that an offer's title must contain.
create table products (
  id           bigint generated always as identity primary key,
  name         text not null,
  search_term  text not null,
  model_code   text not null,
  our_price    numeric not null
);

create table price_snapshots (
  id           bigint generated always as identity primary key,
  product_id   bigint not null references products (id) on delete cascade,
  position     int,
  seller       text,
  title        text,
  price        numeric,
  link         text,
  captured_at  timestamptz not null default now()
);

create index on price_snapshots (product_id, captured_at desc);

-- On hosted Supabase, public tables are reachable through the REST API with the anon key.
-- RLS with no policies blocks that; the SQL editor and pg_cron run as postgres and are unaffected.
alter table products enable row level security;
alter table price_snapshots enable row level security;

insert into products (name, search_term, model_code, our_price) values
  ('Sony WH-1000XM5 Headphones',  'Sony WH-1000XM5', 'WH-1000XM5', 349.00),
  ('Keychron K2 Pro Keyboard',    'Keychron K2 Pro', 'K2 Pro',     109.00),
  ('Dell S2725QC 27" 4K Monitor', 'Dell S2725QC',    'S2725QC',    289.00);

-- Search per row of another table. A plain join would hide the search term from the wrapper
-- (Postgres plans it as a hash join), so pass it in as a literal through dynamic SQL.
create or replace function market_prices(term text) returns setof web.shopping
language plpgsql as $$
begin
  return query execute format('select * from web.shopping where query = %L', term);
end $$;

-- Does an offer's title name this model? Ignores case, spaces and dashes, so
-- 'Sony WH1000XM5' and 'sony wh-1000xm5' both match 'WH-1000XM5'.
create or replace function matches_model(title text, model_code text) returns boolean
language sql immutable as $$
  select strpos(
    regexp_replace(lower(title), '[^a-z0-9]', '', 'g'),
    regexp_replace(lower(model_code), '[^a-z0-9]', '', 'g')
  ) > 0
$$;

-- Live market prices next to your catalog, straight from SQL
select p.name, p.our_price, min(m.price) as market_low, count(*) as matching_offers
from products p
cross join lateral market_prices(p.search_term) m
where matches_model(m.title, p.model_code)
  and m.price between p.our_price * 0.5 and p.our_price * 2
group by p.name, p.our_price;

-- Save current market prices for every product; returns the number of snapshots stored
create or replace function refresh_prices() returns integer
language plpgsql as $$
declare
  n integer;
begin
  insert into price_snapshots (product_id, position, seller, title, price, link)
  select p.id, m.position, m.source, m.title, m.price, m.link
  from products p
  cross join lateral market_prices(p.search_term) m
  where m.price is not null
    and matches_model(m.title, p.model_code);
  get diagnostics n = row_count;
  return n;
end $$;

-- Where each product stands against the market, from its latest snapshot:
--   1. keep offers priced between half and double our price, which drops parts, accessories,
--      bulk lots and price-gouging resellers that still name the model
--   2. take the 10 best-ranked of those and compare our price with their median
--   3. say how many offers that is, so a median of one or two offers isn't trusted blindly
-- security_invoker makes the view respect RLS; by default a view reads tables as its owner.
create or replace view price_position with (security_invoker = true) as
with latest as (
  select s.*
  from price_snapshots s
  join products p on p.id = s.product_id
  where s.captured_at = (
    select max(captured_at) from price_snapshots where product_id = s.product_id
  )
    and s.price between p.our_price * 0.5 and p.our_price * 2
),
top_offers as (
  select *, row_number() over (partition by product_id order by position) as rank
  from latest
)
select
  p.name,
  p.our_price,
  round((percentile_cont(0.5) within group (order by t.price))::numeric, 2)  as market_median,
  min(t.price)                                                               as market_low,
  round(p.our_price - (percentile_cont(0.5) within group (order by t.price))::numeric, 2)
                                                                             as vs_median,
  count(t.price)                                                             as offers,
  case
    when count(t.price) >= 5 then 'high'
    when count(t.price) >= 3 then 'medium'
    else 'low'
  end                                                                        as confidence,
  max(t.captured_at)                                                         as checked_at
from products p
left join top_offers t on t.product_id = p.id and t.rank <= 10
group by p.id, p.name, p.our_price;

select refresh_prices();
select * from price_position;

-- Collect prices every hour (pg_cron must be enabled on the project)
-- create extension if not exists pg_cron;
-- select cron.schedule('refresh-prices', '0 * * * *', 'select refresh_prices()');
