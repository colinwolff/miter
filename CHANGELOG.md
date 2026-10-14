# Changelog

All notable changes to `miter` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0-alpha.2] - 2026-10-14

### Fixed
- **CSS scanner (`polyglot/css`)**: `/* … */` block comments and SCSS `//` end-of-line comments are now masked before selectors and custom properties are read. Every non-newline character inside a comment is replaced with a space `' '` and `\n` is preserved, so byte spans and 1-based line numbers stay exact. Comment prose such as `/* docs/style.css */` or `// theme.config.css` no longer registers phantom class selectors (`.css`, `.config`, `.1`).
- **CSS scanner (`polyglot/css`)**: quoted strings and `url(...)` functions are skipped while tokenising a selector prelude, so a value like `[href^="v1.0.0"]` no longer yields `.1`/`.0`, and a `var(--token)` written inside a comment is no longer indexed as a reference.
- **HTML scanner (`polyglot/html`)**: a start tag now opens only when `<` is immediately followed by an ASCII letter. Comparisons and text arrows (`<-`, `<=`, `< 250`) and closing tags (`</…`) are ignored, so none can register as a custom element. Comments (`<!-- … -->`), declarations (`<!DOCTYPE …>`) and `script`/`style` raw-text bodies are skipped whole, and custom element names must start with a lowercase letter, contain a hyphen and use only `[a-z0-9._-]`.

## [0.1.0-alpha.1] - 2026-10-13

### Added
- **Polyglot AST, Infrastructure & Docs Indexing (`miter index`)**: Deterministic symbol and cross-reference extraction across Rust, TypeScript/JavaScript, Python, SQL, Dockerfile, Kubernetes/Docker Compose YAML, Markdown, and `.env` files backed by `redb`.
- **Surgical Navigation (`miter lookup`, `miter skeleton`, `miter references`)**: Token-efficient interface skeletons, scoped symbol hydration, and reverse call-graph lookups.
- **Structural Analysis Suite (`miter audit`, `miter slice`, `miter impact`, `miter contracts`, `miter clones`)**:
  - Line-ceiling enforcement, dead-symbol detection, visibility downgrade checks, and Tarjan file-level cycle detection.
  - Co-reference AST seam slicing blueprints with visibility elevation suggestions.
  - Multi-hop reverse BFS blast-radius analysis and automatic test selection.
  - Cross-language schema contract verification across SQLite, Rust structs, and TypeScript interfaces.
  - Deterministic 128-bit SimHash AST structural clone detection.
- **Compiler Verification Gate (`miter verify`)**: Subprocess compiler diagnostic parser with optional git-worktree sandboxing.
- **AST-Aware Secret Redaction**: Automatic scrubbing of sensitive identifiers, vendor API key prefixes, and high-entropy secrets at both the storage codec and MCP egress boundaries.
- **MCP Server (`miter mcp`)**: Full JSON-RPC `stdio` server with automatic dirty-file refresh.
- **Packaging & Distribution**: The CLI is published to crates.io as **`miter-cli`** (the bare name `miter` is taken) and installs the `miter` binary; per-target archives with `.sha256` checksums are attached to GitHub Releases and declared in `[package.metadata.binstall]` for `cargo binstall miter-cli`. GitHub Actions runs the fmt/clippy/test/dogfood gate on every push and publishes the release archives on a `v*` tag.
