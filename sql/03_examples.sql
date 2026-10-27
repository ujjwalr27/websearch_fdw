-- websearch_fdw: example queries.

-- Top web results
select position, title, link
from web.search
where query = 'postgres foreign data wrapper';

-- Cheapest offers for a product
select title, price, source
from web.shopping
where query = 'Sony WH-1000XM5'
order by price
limit 5;

-- Localized news
select title, source, published
from web.news
where query = 'electric vehicles'
  and location = 'Austin, Texas, United States'
  and language = 'en';

-- Anything the table doesn't expose as a column is still in `raw`
select title, raw ->> 'delivery' as delivery
from web.shopping
where query = 'Keychron K2 Pro';
