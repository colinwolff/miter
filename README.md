# miter

**A deterministic AST co-processor and Model Context Protocol (MCP) server.**

`miter` indexes a polyglot source tree — Rust, TypeScript/JavaScript, Python, SQL, Dockerfiles,
Kubernetes and Docker Compose manifests, HCL/OpenTofu, Markdown and `.env` files — into an
embedded `redb` database, then answers structural questions about it: symbol hydration, interface
skeletons, reverse call graphs, blast radius, schema contracts and clone detection. No model is on
the query path.

Every answer is a function of the source. Nothing here samples, nothing reads a clock into a
verdict, and nothing asks a network.

---

## Supported languages & formats

| Format | Extensions | What is indexed |
|---|---|---|
| **Rust** | `.rs` | `mod`, `struct`, `enum`, `trait`, `type`, `fn`, methods, fields, doc comments |
| **TypeScript / JavaScript** | `.ts` `.mts` `.cts` `.tsx` `.js` `.jsx` `.mjs` `.cjs` | namespaces, classes, interfaces, type aliases, functions, methods, properties, JSDoc |
| **Python** | `.py` `.pyi` | modules, classes, functions, methods, decorators, docstrings |
| **SQL** | `.sql` | `CREATE TABLE` / `VIEW` / `INDEX` and their columns, plus the tables its `FROM`/`JOIN`/`REFERENCES` clauses name |
| **HTML** | `.html` `.htm` | `id` attributes, custom elements (`<user-card>`), `<template>`s and landmark sections; `class`/`href="#…"`/`for` are indexed as references |
| **CSS / SCSS** | `.css` `.scss` | custom properties (`--brand`), class and id selectors, `@keyframes` and `@layer`; every `var(--brand)` is a reference |
| **Docker** | `Dockerfile`, `Dockerfile.*`, `*.dockerfile`, `Containerfile` | build stages (`FROM … AS`), `ARG`/`ENV` keys, `EXPOSE` ports, `CMD`/`ENTRYPOINT`, and `COPY --from=<stage>` as a reference |
| **Kubernetes** | `.yaml` `.yml` | `kind` + `metadata.name` resources, container specs, `env` keys, and `ConfigMap`/`Secret` `data`/`stringData` keys |
| **Docker Compose** | `.yaml` `.yml` | `services.<name>` and each service's environment keys |
| **HCL / OpenTofu / Terraform** | `.tf` `.tofu` | resources, modules and variables |
| **Markdown** | `.md` `.markdown` | the ATX heading hierarchy (`#` … `######`); `miter skeleton README.md` prints the outline |
| **Environment files** | `.env` `.env.*` | variable keys only — values are never stored |

Every value read from a Dockerfile's `ENV`/`ARG` or a manifest's `env:` block passes through the
AST-aware scrubber before it is recorded, so a credential never reaches `.miter/miter.redb` or an
MCP response. A Kubernetes `secretKeyRef` is recorded as a *reference*
(`secretKeyRef(name.key)`) rather than resolved.

---

## Install

The crate is published as **`miter-cli`** — the bare name `miter` was already taken on
`crates.io` — and every route below installs a binary called **`miter`**.

### Pre-built binaries

Download the archive for your platform from the
[Releases page](https://github.com/colinwolff/miter/releases), verify it against the `.sha256`
beside it, and put `miter` (or `miter.exe`) on your `$PATH`:

| Platform | Archive |
|---|---|
| macOS (Apple silicon) | `miter-v0.1.0-alpha.1-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `miter-v0.1.0-alpha.1-x86_64-apple-darwin.tar.gz` |
| Linux (x86_64) | `miter-v0.1.0-alpha.1-x86_64-unknown-linux-gnu.tar.gz` |
| Windows (x86_64) | `miter-v0.1.0-alpha.1-x86_64-pc-windows-msvc.zip` |

### Via `cargo-binstall` (no compilation)

```bash
cargo binstall miter-cli@0.1.0-alpha.1
```

### From source, via `crates.io`

```bash
cargo install miter-cli --version 0.1.0-alpha.1 --locked
```

### From a checkout

```bash
cargo install --path . --locked
```

---

## Quick start

```bash
# Index the current tree into .miter/miter.redb
miter index .

# Hydrate one declaration: signature, span, members and hierarchy neighbours
miter lookup Engine --context

# Reverse call graph for a symbol
miter references Engine

# Multi-hop blast radius and affected-test selector
miter impact run_stdio_server --depth 3

# Line-count / hygiene / cycle audit
miter audit --threshold 250

# Interface skeleton with bodies stripped (or a Markdown outline)
miter skeleton crates/engine/src/store.rs
miter skeleton README.md

# Structural clone detection
miter clones --path crates --min-lines 20

# Compiler verification (optionally in an ephemeral git worktree)
miter verify --path crates/engine --sandbox --task-id check-1

# Model Context Protocol stdio server for AI coding agents
miter mcp
```

Every command accepts `--json` (or `--format json`) where a machine-readable document is useful.

---

## Commands

| Command | What it does |
|---|---|
| `index <PATH>` | Parse a source tree and write the encoded symbols into the database. `--dirty` re-indexes only changed files. |
| `lookup <SYMBOL>` | Locate a declaration and print its hydrated context (`--context`, `--max-tokens`). |
| `skeleton <PATH>` | Extract a token-compressed interface skeleton with implementation bodies stripped. |
| `references <SYMBOL>` | Inverted reverse lookup of references and call sites. |
| `impact <SYMBOL>` | Multi-hop reverse BFS blast radius plus an affected-unit-test selector. |
| `slice <PATH>` | AST co-reference blueprint and seam-extraction clusters. |
| `audit` | Line ceiling, dead-code/visibility hygiene and Tarjan file cycles. |
| `contracts` | Cross-language SQL / Rust / TypeScript schema verification. |
| `clones` | Standalone 128-bit SimHash AST clone detection. |
| `verify` | Subprocess compiler diagnostic verification (`--sandbox` for an ephemeral worktree). |
| `mcp` | Model Context Protocol stdio server. |

---

## MCP tools

The `miter mcp` server speaks JSON-RPC 2.0 over stdin/stdout and exposes:

`miter_lookup` · `miter_skeleton` · `miter_references` · `miter_slice` · `miter_audit` ·
`miter_impact` · `miter_verify`

Register it with an MCP client by pointing at the binary:

```json
{
  "mcpServers": {
    "miter": { "command": "miter", "args": ["mcp"] }
  }
}
```

---

## Configuration

Configuration is read, in order, from:

1. `<workspace>/.miter/config.toml`
2. `<workspace>/miter.toml`
3. built-in defaults

```toml
# Exclude paths from indexing and auditing (glob syntax, workspace-relative).
exclude = ["vendor/**", "**/generated/**"]

[indexer]
exclude = ["**/fixtures/**"]

[audit]
threshold = 500
exclude = ["crates/**/tests/**"]
```

`.gitignore` rules and a baseline set of secret/credential globs (`*.pem`, `*.key`, `id_rsa*`,
`credentials.json`, …) are always applied on top of the configured patterns.

---

## Workspace layout

```text
.miter/
  miter.redb      ← the workspace database: symbols, declarations, references, file hashes
```

`miter` finds its workspace by walking up from the current directory to the nearest `.miter/`
or `.git/` marker. Point `--db` at a different file to work in parallel.

---

## Crates

| Crate | Responsibility |
|---|---|
| `miter-ast` | Tree-sitter ingestion: functions, inputs and return types, with their provenance and nesting depth. |
| `miter-engine` | Storage engine, polyglot model, analysis (audit, slice, clones, contracts, cycles, hygiene, impact, references, skeleton), hydration and secret redaction. |
| `miter-storage` | Canonical `redb` tables, schema versioning and auto-purge recovery. |
| `miter-harness` | Deterministic compiler verification and the ephemeral-worktree sandbox. |
| `miter-mcp` | Model Context Protocol stdio server. |
| `miter-cli` (root) | The CLI binary, installed as the `miter` command. |

---

## Design invariants

- **Determinism.** Two runs over the same tree produce the same numbers. Ids are content-addressed
  (`fnv1a64` of `path:line:name`); neighbour ordering is fully specified.
- **No heap allocation in hot paths** where a fixed-size array will do.
- **Bounded output.** Hydration spends at most `--max-tokens`; the render sheds whole lines rather
  than truncating one.
- **Refusal over fabrication.** A missing index is reported with the command that builds it; a
  symbol that does not exist is `NotFound`; a record with the wrong magic or version is refused.
- **Files stay small.** No source file exceeds 250 lines; `miter audit` enforces it and modules
  are decomposed before they reach it.

---

## License

Licensed under the [Apache License, Version 2.0](LICENSE). See [`NOTICE`](NOTICE) for attribution.
