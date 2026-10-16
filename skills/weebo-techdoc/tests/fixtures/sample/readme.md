# sample

A tiny item store: an axum API (`api/`) backed by Postgres, and a TypeScript
web client (`web/`) that lists and creates items. Creating an item needs a
bearer token, checked by the `auth-core` library crate (`crates/auth-core/`).
