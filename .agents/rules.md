# Agent Operating Rules

These rules govern autonomous and assisted engineering tasks in this workspace. `miter` serves as
the deterministic co-processor: the model provides reasoning and code synthesis, while `miter`
enforces AST contracts and compiler correctness.

---

## 1. Modularity & File Size Invariants

- **Hard File Limit**: No newly created module or refactored source file may exceed **250 lines**.
- **Audit Ceiling**: Files exceeding **500 lines** are architectural technical debt and must be
  decomposed using `miter slice`. Files exceeding **800 lines** are critical violations.
- **Single Responsibility**: Every crate sub-module must represent a single coherent domain,
  adapter, or trait implementation. Never aggregate unrelated utilities into a monolithic
  `utils.rs` or bloated `mod.rs`.

---

## 2. AST-First Code Inspection

- **Mandatory AST Resolution**: prefer `miter lookup`, `miter skeleton` and `miter references` to
  discover symbols, signatures and call sites before reading source.
- **Routing Rules**:
  - **Surgical Symbols**: `miter lookup <symbol> --context` retrieves signatures, spans and the
    geodesic neighbour graph (~120 tokens).
  - **Interface Stripping**: `miter skeleton <path>` returns a token-compressed API surface with
    bodies stripped.
  - **Reverse Call Graph**: `miter references <symbol>` returns AST-precise call sites.
- **`read_file` Scope**: restricted to the lines around a planned edit, and only after the target
  contract has been resolved via `miter lookup`.
- **Unindexed Files Exception**: if a file was created during the active session and is not yet in
  `miter.redb`, run `miter index .` or inspect it directly.

---

## 3. Closed-Loop Verification Invariant

- **Mandatory Compiler Gate**: immediately after writing or editing code in any crate, run
  `miter verify --path <crate_path>` before declaring a task complete.
- **No Terminal Build Flooding**: do not run raw `cargo build` for basic syntax/type validation;
  `miter verify` returns isolated, sliced diagnostic spans at sub-second latency.
- **Diagnostic Repair Loop**: when `miter verify` returns a `DiagnosticFailure`:
  1. Read the primary diagnostic span coordinates (`file:line:col`) and error code.
  2. Inspect the offending symbol via `miter lookup`.
  3. Apply the targeted patch.
  4. Re-run `miter verify` to confirm resolution.

---

## 4. Large File Decomposition Workflow

- Run `miter slice <path>` to obtain an AST co-reference blueprint and extraction clusters.
- Extract one cluster at a time, re-exporting historical items through `pub use` so existing
  callers compile without changes.
- Re-run `miter verify` and `miter audit --threshold 250` after each extraction.

---

## 5. Commit Discipline

- Every logical step is committed individually with a Conventional Commit message
  (`feat(...)`, `fix(...)`, `refactor(...)`, `docs(...)`, `chore(...)`).
- Never combine unrelated refactors or features into one commit.
