# Changelog

All notable changes to `miter` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
