# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

aborg (Audiobook Organizer) is a Rust CLI that renames and reorganizes an audiobook collection. It reads a `metadata.json` (as produced by Audiobookshelf) plus data parsed from filenames, then copies or moves files into a directory/file structure defined by user-supplied Handlebars templates. Still beta.

## Tech Stack

- **Rust** (edition 2024)
- **clap** (derive) for the CLI, **handlebars** for path/file templating
- **lofty** for reading audio tags, **walkdir** for traversal, **regex** for filename parsing
- **serde**/**serde_json** for `metadata.json`, **colored** for output

## Commands

```bash
cargo build --release   # Build
cargo test              # Run tests
./run.sh                # Local test run against ./test_bak fixtures into ./output
./build.sh              # Cross-compile a static x86_64 Linux musl binary (needs zig + cargo-zigbuild)

# Typical usage (preview first with --dry-run)
cargo run -- --source <SRC> --destination <DST> --action 2 --dry-run
```

`--action`: `0` copy, `1` move keeping the source directory, `2` move and delete the emptied source directory.

## Directory Overview

`src/`:
- `main.rs`: CLI definition (clap args), traversal, and the copy/move/delete orchestration honoring `--dry-run`.
- `metadata.rs`: parses `metadata.json` and exposes the fields available to templates.
- `track.rs`: per-file audio handling (tags via lofty, filename parsing, ordering/numbering).
- `schema.rs`: Handlebars rendering of the path and file schemas.

Templates expose fields like `author`, `series`, `title`, `book_number_with_zeros`, `file_number_with_zeros` (see README for the full list and defaults).

## Model Selection

- **Claude Fable 5 (`claude-fable-5`):** filesystem move/delete safety and dry-run correctness, filename-parsing and edge-case metadata handling, and template-engine changes.
- **Claude Opus 4.8 (`claude-opus-4-8`):** default for new CLI options, actions, or metadata sources.
- **Claude Sonnet 5 (`claude-sonnet-5`):** routine Rust changes, small fixes, unit tests.
- **Claude Haiku 4.5 (`claude-haiku-4-5`):** quick lookups, README and help-text edits, boilerplate.
