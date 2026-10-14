# AGENTS.md — miter

Operational instructions and runtime guardrails for autonomous coding agents working in this
repository.

## Project Mission

`miter` is a deterministic AST co-processor and Model Context Protocol (MCP) server. It indexes a
polyglot source tree into an embedded `redb` database and answers structural questions about it:
symbol hydration, interface skeletons, reverse call graphs, blast radius, schema contracts and
clone detection. There is no model on the query path and no network access.

The formats it reads: Rust, TypeScript / TSX / JavaScript, Python, SQL, Dockerfiles (`Dockerfile`,
`Dockerfile.*`, `*.dockerfile`, `Containerfile`), Kubernetes and Docker Compose YAML,
HCL/OpenTofu/Terraform, Markdown headings and `.env` key files. The list lives in
`polyglot::{language,dispatch}`; a new format is a new extractor there plus a test, not a new
switch in a caller.

## Architecture & Crate Boundaries

Cargo workspace. Do not blur crate responsibilities:

- `crates/ast_indexer` (`miter-ast`): Tree-sitter ingestion of Rust / TypeScript / TSX /
  JavaScript / Python. Owns the grammar table (`grammar_for`) and reports each declaration's
  provenance and nesting depth. No persistence, no analysis.
- `crates/engine` (`miter-engine`): the storage engine and every analysis pass — polyglot model,
  audit, slice/cluster, clones (SimHash), contracts, cycles, hygiene, impact, references,
  skeleton, hydration and secret redaction. Persistence codec lives here; it is built on
  `miter-storage`'s tables.
- `crates/storage` (`miter-storage`): canonical `redb` tables, `ENGINE_STORAGE_VERSION`, the
  `.miter/` workspace heuristic, the file-hash ledger and auto-purge recovery. No grammar, no
  analysis.
- `crates/harness` (`miter-harness`): one deterministic compiler-verification backend
  (`cargo check` / `tsc --noEmit`) plus the ephemeral git-worktree sandbox. It runs an external
  checker and reports what it said; it never scores or ranks.
- `crates/mcp` (`miter-mcp`): the JSON-RPC 2.0 stdio server. Transports only; owns no thresholds
  or verdicts.
- Root package (`src/`): the `miter` CLI binary.

## Invariant Rules (DO NOT BREAK)

### 1. Determinism

- Symbol ids are content-addressed: `fnv1a64(path:line:name) | 1` (`seed_from_text`).
- A declaration sits at a hierarchy level (`0..=4`), read off the syntax tree. Neighbours are the
  declarations within `HYDRATION_LEVEL_BAND` levels of the anchor.
- Neighbour ordering in hydration is fully specified: depth closeness, then name affinity, then
  distance, then id. Never sort by an unordered collection's iteration order.
- No clock, RNG or network call may influence a command's output.

### 2. Persistence & Schema

- One `redb` file per workspace at `.miter/miter.redb` (`miter_storage::default_db_path`).
- Every record is little-endian and self-describing: a 5-byte `MITER` magic, a version byte, then
  fixed fields and length-prefixed text (`miter_engine::codec`).
- The decoder never panics: a wrong magic, a wrong version, a truncated record or malformed UTF-8
  is a typed `DecodeError`. Bump `codec::VERSION` (and `ENGINE_STORAGE_VERSION`) together when a
  record layout changes.
- The table names are a public interface. A rename makes every database in the wild unreadable,
  which is a purge, not a refactor.

### 3. Bounded Output

- Hydration spends at most `--max-tokens`; the render sheds whole lines (farthest neighbour first,
  then the last public member) rather than truncating one.
- A budget too small for the declaration's own header is refused (`HydrateError::EmptyBudget`),
  never met with a truncated signature.
- `estimate_tokens` takes the larger of four-characters-per-token and whitespace-field counting.

### 4. Refusal Over Fabrication

- A missing index reports the command that builds it (`miter index .`).
- A symbol the index does not hold is `HydrateError::NotFound`, distinct from `IndexEmpty`.
- A verdict and a harness failure are different returns: `verify` returns `Ok(outcome)` for a
  check that *ran* (including one that rejected the tree) and `Err(HarnessError)` only when the
  harness itself failed.
- The sandbox jail is not advisory: `apply_patch` refuses absolute paths and `..` before
  normalisation.

### 5. Security

- Secret redaction is AST-aware: high-entropy literals and sensitive identifiers are scrubbed
  before a payload leaves the process (`engine::security`).
- The MCP server writes protocol frames to stdout and every log line to stderr; stdout is
  reserved for newline-delimited JSON-RPC.

### 6. Modularity, Anti-Monolith Guardrails & Atomic Commits

- **File Length Ceiling**: no source file (Rust, TypeScript, HTML/CSS) may reach **250 lines**.
  `miter audit` enforces it (`line_count >= threshold` is a violation) and CI runs it at 250;
  decompose proactively into focused submodules (`foo/{mod, types, error, cases}.rs`).
- **Zero Breaking Changes on Decomposition**: re-export historical items through `pub use` so
  existing callers compile without changes.
- **Atomic, Bisectable Commits**: never combine sprawling changes into one commit. Every logical
  step is committed individually with a Conventional Commit message (`refactor(...)`, `feat(...)`,
  `fix(...)`, `docs(...)`, `chore(...)`).

### 7. Versioning & Releases

- One version in `[workspace.package] version`, mirrored across every internal pin.
- SemVer: minor bump for any public interface change, patch for additive backwards-compatible
  changes.
- A version bump and its `CHANGELOG.md` entry land in the same commit. No `Unreleased` heading.

### 8. Tiered Verification

- **Tier 1 (per task)**: `cargo check --workspace --all-targets` plus a targeted
  `cargo test -p <crate> <module>` for the unit under edit.
- **Tier 2 (milestone)**: `cargo nextest run --workspace --lib`.
- **Tier 3 (release gate, once per version bump)**:
  1. `./scripts/test.sh`
  2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- In a sandbox where `sccache` cannot run, prefix with `RUSTC_WRAPPER=""`.

## Common Commands

```bash
cargo check --workspace --all-targets
cargo test -p miter-engine analysis
cargo nextest run --workspace --lib
./scripts/test.sh
RUSTFLAGS="-C target-cpu=native" cargo build --release

cargo run --release -- index .
cargo run --release -- audit --threshold 250
cargo run --release -- lookup Engine --context
cargo run --release -- mcp
```

## Definition of Done for Any Task

1. The workspace compiles with zero warnings: `cargo check --workspace --all-targets`.
2. All invariants above hold.
3. Targeted tests for the changed module pass.
4. If a public interface changed, the version and `CHANGELOG.md` are updated in the same commit.
5. Changes are committed as targeted, atomic commits with conventional messages.
