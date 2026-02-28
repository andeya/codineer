# CODINEER.md

This file provides guidance to Codineer when working with code in this repository.

## Detected stack
- Languages: Rust.

## Verification
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

## Repository shape
- Standard Rust workspace with `crates/` containing all library and binary crates.

## Working agreement
- Prefer small, reviewable changes.
- Keep shared defaults in `.codineer.json`; reserve `.codineer/settings.local.json` for machine-local overrides.
- Do not overwrite existing `CODINEER.md` content automatically; update it intentionally when repo workflows change.
