-- Move magic links to Redis; remove the Postgres table.
DROP TABLE IF EXISTS magic_links;
