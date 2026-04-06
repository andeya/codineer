# CODINEER.md

This file provides guidance to Codineer (codineer.dev) when working with code in this repository.

## Detected stack
- Languages: Rust.
- Frameworks: none detected from the supported starter markers.

## Verification
- Run Rust verification from the repo root: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`

## Working agreement
- Prefer small, reviewable changes and keep generated bootstrap files aligned with actual repo workflows.
- Keep shared defaults in `.codineer/settings.json`; reserve `.codineer/settings.local.json` for machine-local overrides.
- Do not overwrite existing `.codineer/CODINEER.md` content automatically; update it intentionally when repo workflows change.
