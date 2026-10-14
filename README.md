# miter

**Give your AI coding agent X-ray vision instead of `grep`.**

When AI coding agents (Claude Code, Cline, Cursor, Windsurf) work on a growing codebase, they waste
thousands of tokens reading entire files, break builds when splitting large modules, miss schema
mismatches between SQL/Rust/TypeScript, and risk reading raw secrets from `.env` and Docker files.
`miter` is a fast, local CLI and Model Context Protocol (MCP) server that gives both you and your
AI agent a live structural map of your project across **13 code, config, and documentation
formats** — with automatic secret redaction built in.

There is no model on the query path, no network call and no cloud service. `miter index .` builds
`.miter/miter.redb` once — 1.4 s for this repository's 190 files — and every answer after that is a
pure function of your source tree: the same answer on every machine, on every run.

```bash
miter index .                           # build the map (once, or per commit)
miter skeleton src/api/handlers.rs      # 800-line file → its signatures
miter lookup Engine --context           # one symbol, its members, its neighbours
```

---

## Why `miter`?

Five things go wrong when an agent is asked to change code whose shape it cannot see.

### 1. Stop burning your context window

**`miter skeleton` · `miter lookup`**

| Without `miter` | With `miter` |
|---|---|
| Your agent reads an entire 800-line file just to see the function signatures, spending 80%+ of its context window on bodies it will never use. | `miter skeleton` strips function bodies and returns a compact map of signatures and line numbers in milliseconds. `miter lookup <Symbol>` fetches just the target symbol and its immediate neighbours. |

### 2. Split large files without breaking the build

**`miter slice` · `miter audit`**

| Without `miter` | With `miter` |
|---|---|
| Asking an AI agent to break up a bloated file usually results in circular dependencies, broken imports, and missing `pub` visibility modifiers. | `miter audit` enforces your file-length ceiling (e.g. 250 lines) and flags dead code, while `miter slice <file>` computes the cleanest seam to split a file and lists the exact symbols that need `pub(crate)` elevation before you touch a line of code. |

### 3. Catch full-stack type drift before runtime

**`miter contracts`**

| Without `miter` | With `miter` |
|---|---|
| You add a column to a `.sql` migration and update your Rust `struct`, but forget to update the TypeScript `interface` on the frontend — and find out in production. | `miter contracts` compares schemas across SQL tables, Rust structs, and TypeScript interfaces and flags missing or mismatched fields immediately. |

### 4. Know what will break before you edit

**`miter impact` · `miter references`**

| Without `miter` | With `miter` |
|---|---|
| Text `grep` returns dozens of noisy comments and unrelated substrings, or misses multi-hop callers entirely. | `miter impact <Symbol>` traces the reverse call graph to show every downstream file and affected test that touches your change — and prints the exact `cargo test <name>` command to run for each one. |

### 5. Never leak API keys to an LLM

**Built in — no flags, no configuration**

| Without `miter` | With `miter` |
|---|---|
| If an agent inspects `.env`, a `Dockerfile`, or Kubernetes YAML manifests, raw credentials and tokens enter the LLM prompt context. | Every indexed signature, body, and MCP tool response passes through AST-aware secret scrubbing — replacing sensitive keys, `sk_live_...` / `ghp_...` tokens, and high-entropy strings with `[REDACTED_SECRET]` automatically. |

Ordinary configuration survives: `ENV PORT=8080` and `ARG CARGO_PROFILE=release` are still indexed
and still searchable. Only values that look like credentials are replaced, and the scrubber runs at
both boundaries — before a value is written to the database, and again before it leaves the process
as an MCP response.

---

## See It in Action

### 1. Outline a file — and redact its secrets — `miter skeleton`

A `Containerfile` that declares a registry token and a credentialed database URL. Every declaration
is there; neither secret is:

```console
$ miter skeleton Containerfile
    1  STAGE builder FROM rust:1.90
    2  ARG CARGO_PROFILE=release
    3  ARG REGISTRY_TOKEN=[REDACTED_SECRET]
    4  ENV DATABASE_URL=[REDACTED_SECRET]
    5  ENV PORT=8080
    5  ENV LOG_LEVEL=info
    6  EXPOSE 8080
    7  CMD ["./miter", "mcp"]
```

The same scrubber covers Compose services and Kubernetes manifests:

```console
$ miter skeleton docker-compose.yml
    2  SERVICE api (image: miter:0.1.0-alpha.1)
    8  SERVICE db (image: postgres:16)
    5  ENV DATABASE_URL=[REDACTED_SECRET]
    6  ENV STRIPE_SECRET_KEY=[REDACTED_SECRET]
    7  ENV LOG_LEVEL=info
```

On source code the same command strips bodies instead of values (output trimmed to the
declarations):

```console
$ miter skeleton src/cli/slice.rs
impl SliceFormat {
    /// Parses format string (`human` or `json`).
    #[must_use]
    pub fn parse(s: &str) -> Self { /* ... */ }
}

/// Runs the `miter slice` command.
pub fn run_slice(path: &Path, target_lines: usize, format: SliceFormat) -> Result<(), CliError> { /* ... */ }

fn slice_json(blueprints: &[ExtractionBlueprint], path: &Path) -> Json { /* ... */ }

fn render_slice_human(blueprints: &[ExtractionBlueprint], path: &Path, target_lines: usize) { /* ... */ }
```

### 2. Plan a safe file split — `miter slice`

A handler module that has grown past the ceiling. `miter slice` names the seam, the symbols that
move, and the exact visibility each one needs to keep compiling:

```console
$ miter slice crates/mcp/src/handler/analysis.rs
EXTRACTION BLUEPRINTS: Found 1 cluster(s) for crates/mcp/src/handler/analysis.rs targeting ≤250 lines:

=== Cluster #1 -> crates/mcp/src/handler/analysis/handle_audit.rs ===
  Source Lines: 10-181 (172 lines)
  Symbols to Move: handle_audit, handle_slice, handle_skeleton, handle_references, handle_impact
  Parent mod decl: mod handle_audit;
  Suggested Exports: pub(crate) handle_audit, pub(crate) handle_slice, pub(crate) handle_skeleton, pub(crate) handle_references, pub(crate) handle_impact
  Est. Remaining Lines in Parent: 13
```

Hand that to your agent and the extraction is mechanical: create `analysis/handle_audit.rs`, move
lines 10–181 into it, add `mod handle_audit;` to the parent, and re-export the five handlers as
`pub(crate)` — no circular imports, no guessed visibility. `miter audit` is the check that the file
stayed under the ceiling afterwards.

### 3. Catch SQL ↔ Rust ↔ TypeScript drift — `miter contracts`

Three declarations of the same entity, written months apart. The frontend never got `created_at`:

```sql
-- db/schema.sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY, email TEXT NOT NULL,
    display_name TEXT NOT NULL, created_at TEXT NOT NULL
);
```

```rust
// src/models.rs
pub struct User { pub id: i64, pub email: String, pub display_name: String, pub created_at: String }
```

```ts
// web/types.ts
export interface User { id: number; email: string; displayName: string }
```

```console
$ miter contracts
Polyglot Contract Verification across SQL, Rust, and TypeScript
================================================================================
Verified Multi-Language Entities: 1
Entities with Contract Drift:     1

ENTITY                   FIELD                    DRIFT STATUS
--------------------------------------------------------------------------------
user                     created_at               in [rust, sql] but missing in [typescript]
```

The match is name-based and language-aware: `users` (SQL), `User` (Rust) and `User` (TypeScript)
all canonicalize to `user`, and `displayName` canonicalizes to `display_name`, so the only field
reported is the one that is genuinely absent.

---

## Install

The crate is published on crates.io as **`miter-cli`** — the bare name `miter` was already taken —
and every route below installs a binary called **`miter`**.

### Pre-built binaries (GitHub Releases)

Download the archive for your platform from the
[Releases page](https://github.com/colinwolff/miter/releases), check it against the `.sha256`
published beside it, and put `miter` (or `miter.exe`) on your `$PATH`:

| Platform | Archive |
|---|---|
| macOS (Apple silicon) | `miter-v0.1.0-alpha.1-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `miter-v0.1.0-alpha.1-x86_64-apple-darwin.tar.gz` |
| Linux (x86_64) | `miter-v0.1.0-alpha.1-x86_64-unknown-linux-gnu.tar.gz` |
| Windows (x86_64) | `miter-v0.1.0-alpha.1-x86_64-pc-windows-msvc.zip` |

```bash
tar -xzf miter-v0.1.0-alpha.1-x86_64-unknown-linux-gnu.tar.gz
install -m 755 miter ~/.local/bin/miter      # or any other directory on $PATH
```

### Via `cargo-binstall` (no compilation)

```bash
cargo binstall miter-cli@0.1.0-alpha.1
```

### From source, via `crates.io`

The package is `miter-cli`; the command it installs is `miter`.

```bash
cargo install miter-cli --version 0.1.0-alpha.1 --locked
```

From a checkout of this repository, the same install without going through the registry:

```bash
cargo install --path . --locked
```

---

## Quick Start (CLI & MCP Server)

### The 30-second CLI workflow

```bash
miter index .                       # build .miter/miter.redb for this workspace
miter audit                         # file-length ceiling, dead code and import cycles
miter skeleton src/api/handlers.rs  # signatures only — bodies stripped
miter lookup Engine --context       # one symbol: signature, members, hierarchy neighbours
```

`miter index .` walks up from the current directory to the nearest `.miter/` or `.git/` marker and
writes `<root>/.miter/miter.redb`. Pass `--db <PATH>` to keep a second index beside it, `--dirty`
to re-index only what git says changed, and `--json` when you want a document rather than a report.
Every read command accepts `--db` too, so two agents can work in parallel instead of fighting over
one file.

A database is one writer at a time: `index` takes an exclusive lock, while `lookup`, `references`,
`impact` and `contracts` take a shared one and run concurrently.

### Wire it into your agent (MCP)

`miter mcp` speaks JSON-RPC 2.0 over stdin/stdout. Add it to your client's MCP configuration:

```json
{
  "mcpServers": {
    "miter": {
      "command": "miter",
      "args": ["mcp"]
    }
  }
}
```

| Client | Where that block goes |
|---|---|
| **Claude Code** | `claude mcp add miter -- miter mcp`, or the same `mcpServers` block in `.mcp.json` |
| **Cline** | `cline_mcp_settings.json` |
| **Cursor** | `.cursor/mcp.json` (project) or `~/.cursor/mcp.json` (global) |
| **Windsurf** | `~/.codeium/windsurf/mcp_config.json` |

Every tool call that reads the index refreshes dirty files (mtime and git status) before it
answers, so an edit your agent just made is visible to the next tool call without a manual
`miter index .`. Protocol frames go to stdout and every log line to stderr, so nothing a server
logs can corrupt a response.

---

## Supported Languages & Formats

One index, thirteen formats. The first five are read with a tree-sitter grammar; the rest are
declarative files whose declarations are read directly.

| # | Format | Extensions | What is indexed |
|---|---|---|---|
| 1 | **Rust** | `.rs` | `mod`, `struct`, `enum`, `trait`, `type`, `fn`, methods, fields, doc comments |
| 2 | **TypeScript** | `.ts` `.mts` `.cts` `.d.ts` | namespaces, classes, interfaces, type aliases, functions, methods, properties, JSDoc |
| 3 | **TSX** | `.tsx` | the TypeScript surface plus JSX components |
| 4 | **JavaScript** | `.js` `.jsx` `.mjs` `.cjs` | classes, functions, methods, properties, JSDoc (read with the TSX grammar) |
| 5 | **Python** | `.py` `.pyi` | modules, classes, functions, methods, decorators, docstrings |
| 6 | **SQL** | `.sql` | `CREATE TABLE` / `VIEW` / `INDEX` and their columns, plus the tables its `FROM`/`JOIN`/`REFERENCES` clauses name |
| 7 | **HTML** | `.html` `.htm` | `id` attributes, custom elements (`<user-card>`), `<template>`s and landmark sections; `class`, `href="#…"` and `for` are indexed as references |
| 8 | **CSS / SCSS** | `.css` `.scss` | custom properties (`--brand`), class and id selectors, `@keyframes` and `@layer`; every `var(--brand)` is a reference |
| 9 | **Dockerfile / Containerfile** | `Dockerfile`, `Dockerfile.*`, `Containerfile`, `*.dockerfile` | build stages (`FROM … AS`), `ARG`/`ENV` keys, `EXPOSE` ports, `CMD`/`ENTRYPOINT`, and `COPY --from=<stage>` as a reference |
| 10 | **Kubernetes & Docker Compose** | `.yaml` `.yml` | `kind` + `metadata.name` resources and their container specs; `services.<name>` and each service's environment keys; `ConfigMap`/`Secret` `data`/`stringData` keys |
| 11 | **HCL / OpenTofu / Terraform** | `.tf` `.tofu` | resources, modules and variables |
| 12 | **Markdown** | `.md` `.markdown` | the ATX heading hierarchy (`#` … `######`); `miter skeleton README.md` prints the outline |
| 13 | **Environment files** | `.env` `.env.*` | variable keys only — values are never stored |

Every value read from a Dockerfile's `ENV`/`ARG` or a manifest's `env:` block passes through the
AST-aware scrubber before it is recorded, so a credential never reaches `.miter/miter.redb` or an
MCP response. A Kubernetes `secretKeyRef` is recorded as a *reference*
(`secretKeyRef(name.key)`) rather than resolved.

`miter audit` counts lines for every format except Markdown and `.env`: a long README is not a
monolith to decompose.

---

## CLI Commands & MCP Tools Reference

Every command is available on the CLI. The seven an agent needs while editing are also exposed as
MCP tools, so a client never has to shell out.

| Command | What it does | MCP tool |
|---|---|---|
| `index <PATH>` | Parse a source tree and write the encoded symbols into the database. `--dirty` re-indexes only changed files. | — |
| `lookup <SYMBOL>` | Locate a declaration and print its hydrated context (`--context`, `--max-tokens`). | `miter_lookup` (adds `scope=symbol\|module\|package`) |
| `skeleton <PATH>` | Extract a token-compressed interface skeleton with implementation bodies stripped. | `miter_skeleton` |
| `references <SYMBOL>` | Inverted reverse lookup of references and call sites. | `miter_references` |
| `impact <SYMBOL>` | Multi-hop reverse BFS blast radius plus an affected-unit-test selector. | `miter_impact` |
| `slice <PATH>` | AST co-reference blueprint and seam-extraction clusters. | `miter_slice` |
| `audit` | Line ceiling, dead-code/visibility hygiene and Tarjan file cycles. | `miter_audit` |
| `contracts` | Cross-language SQL / Rust / TypeScript schema verification. | — |
| `clones` | Standalone 128-bit SimHash AST clone detection. | — |
| `verify` | Subprocess compiler diagnostic verification (`--sandbox` for an ephemeral worktree). | `miter_verify` |
| `mcp` | Model Context Protocol stdio server. | — |

Add `--json` (or `--format json`) to any analysis command for a machine-readable document. Exit
codes are `0` for a completed run and `1` for a refusal — including a verdict from `verify` that
rejected the tree, which is a result rather than a failure of the harness.

---

## Workspace Architecture

### Where the index lives

```text
.miter/
  miter.redb      ← the workspace database: symbols, declarations, references, file hashes
  config.toml     ← optional, and the one file worth committing
```

`miter` finds its workspace by walking up from the current directory to the nearest `.miter/` or
`.git/` marker. Point `--db` at a different file to work in parallel.

The database is a build artifact: it is derived entirely from the source tree, and `miter index .`
rebuilds it from scratch whenever it is missing. Records are little-endian and self-describing — a
`MITER` magic, a version byte, then fixed fields and length-prefixed text — and a record whose
magic or version does not match is refused with a typed error rather than misread, and the store
purges and rebuilds a file it cannot read instead of answering from it.

### Configuration

Configuration is read, in order, from:

1. `<workspace>/.miter/config.toml`
2. `<workspace>/miter.toml`
3. built-in defaults

```toml
exclude = ["vendor/**", "**/generated/**"]

[indexer]
exclude = ["**/fixtures/**"]

[audit]
threshold = 250
exclude = ["crates/**/tests/**"]
```

A top-level `exclude` applies to both passes; `[indexer]` and `[audit]` narrow it per pass, and
`[audit] threshold` is the file-length ceiling `miter audit` enforces. `.gitignore` rules and a
baseline set of secret/credential globs (`*.pem`, `*.key`, `id_rsa*`, `credentials.json`, …) are
always applied on top of the configured patterns.

### Crates

| Crate | Responsibility |
|---|---|
| `miter-ast` | Tree-sitter ingestion: functions, inputs and return types, with their provenance and nesting depth. |
| `miter-engine` | Storage engine, polyglot model, analysis (audit, slice, clones, contracts, cycles, hygiene, impact, references, skeleton), hydration and secret redaction. |
| `miter-storage` | Canonical `redb` tables, schema versioning and auto-purge recovery. |
| `miter-harness` | Deterministic compiler verification and the ephemeral-worktree sandbox. |
| `miter-mcp` | Model Context Protocol stdio server. |
| `miter-cli` (root) | The CLI binary, installed as the `miter` command. |

### Design invariants

- **Determinism.** Two runs over the same tree produce the same numbers. Ids are content-addressed
  (`fnv1a64` of `path:line:name`); neighbour ordering is fully specified.
- **No model on the query path.** Nothing samples, nothing reads a clock into a verdict, and
  nothing asks a network.
- **Bounded output.** Hydration spends at most `--max-tokens`; the render sheds whole lines rather
  than truncating one.
- **Refusal over fabrication.** A missing index is reported with the command that builds it; a
  symbol that does not exist is `NotFound`; a record with the wrong magic or version is refused.
- **Files stay small.** No source file exceeds 250 lines; `miter audit` enforces it and modules are
  decomposed before they reach it.

---

## License

Licensed under the [Apache License, Version 2.0](LICENSE). See [`NOTICE`](NOTICE) for attribution.
