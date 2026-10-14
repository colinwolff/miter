#!/usr/bin/env bash
#
# The workspace test gate.
#
#   ./scripts/test.sh                          # fast gate: every non-ignored test, plus the doctests
#   ./scripts/test.sh --full                    # release gate: builds and runs in release
#   ./scripts/test.sh -p engine                 # a package instead of the workspace (and no doctests)
#   ./scripts/test.sh -E 'test(verifier)'    # anything else is forwarded to the runner
#   MITER_TEST_RUNNER=cargo ./scripts/test.sh    # force the `cargo test` path
#
# The runner is `cargo-nextest` when it is on `PATH` and `cargo test` otherwise, and both paths
# run the *same* set of tests:
#
# * `.config/nextest.toml` is the nextest profile: no retries, one test per core, every failure
#   reported, and every failure surfaced rather than the first.
# * nextest does not run doctests, so this script runs them with `cargo test --doc` afterwards —
#   the same five the `cargo test` path covers inside its normal run. Without that step the
#   nextest gate would silently be a smaller gate than the fallback, which is exactly the kind of
#   difference a "preferred runner" must not have.
#
# `--full` is the release gate: it builds and runs in release, where the compiler-verification
# fixtures are fast enough to be worth running against optimised code.
#
set -euo pipefail
cd "$(dirname "$0")/.."

full=0
forward=()
for arg in "$@"; do
    case "$arg" in
        --full) full=1 ;;
        *) forward+=("$arg") ;;
    esac
done

# `--workspace` is what makes this the *gate*, but cargo refuses it together with a package
# selection (`-p/--package`, `--exclude`), so a forwarded one replaces the scope instead of
# fighting it. A forwarded *test filter* (a bare name, or nextest's `-E`) keeps the workspace.
scope=(--workspace)
for arg in ${forward[@]+"${forward[@]}"}; do
    case "$arg" in
        -p | --package | --exclude) scope=() ;;
    esac
done

# `release` / `ignored` are the two flags that distinguish the gates; they are built once and
# passed to whichever runner the machine has.
release=()
ignored=()
if ((full)); then
    release+=(--release)
fi

runner="${MITER_TEST_RUNNER:-auto}"
if [[ "$runner" == "auto" ]]; then
    if command -v cargo-nextest >/dev/null 2>&1; then
        runner="nextest"
    else
        runner="cargo"
    fi
fi

# Optional fast linker: `MITER_FAST_LINKER=lld|mold|auto|none`. `auto` (the default) uses whichever
# of `ld.lld`/`mold` is on `PATH` and says nothing when neither is — the platform's own linker is
# what rustc already picks (Apple `ld64` via `cc` on macOS), and it is only ever *slower* than lld
# or mold, never wrong. An explicitly requested linker that is not installed is reported and the
# run continues on the default rather than failing the gate over an optimisation.
#
# The flag travels in `CARGO_TARGET_<TRIPLE>_RUSTFLAGS` because cargo has no per-profile linker
# setting; a target-scoped environment variable *replaces* the rustflags `.cargo/config.toml`
# declares for that target, so the platform's own flags are repeated here. Expect a one-time
# rebuild the first time a linker is switched on, and another when it is switched off: the flags
# are part of every unit's fingerprint.
fast_linker="${MITER_FAST_LINKER:-auto}"
if [[ "$fast_linker" == "auto" ]]; then
    fast_linker="none"
    for candidate in lld mold; do
        if command -v "ld.$candidate" >/dev/null 2>&1 || command -v "$candidate" >/dev/null 2>&1; then
            fast_linker="$candidate"
            break
        fi
    done
fi
if [[ "$fast_linker" != "none" ]] &&
    ! command -v "ld.$fast_linker" >/dev/null 2>&1 &&
    ! command -v "$fast_linker" >/dev/null 2>&1; then
    echo "=== $fast_linker is not on PATH: running with the platform linker"
    fast_linker="none"
fi
if [[ "$fast_linker" != "none" ]]; then
    case "$(uname -s)" in
        Darwin) fast_linker_flags="-C target-cpu=native -C link-arg=-mmacosx-version-min=13.1" ;;
        *) fast_linker_flags="-C target-cpu=native" ;;
    esac
    fast_linker_var="CARGO_TARGET_$(rustc -vV | awk '/^host: /{print $2}' | tr 'a-z-' 'A-Z_')_RUSTFLAGS"
    export "$fast_linker_var=$fast_linker_flags -C link-arg=-fuse-ld=$fast_linker"
    echo "=== fast linker: $fast_linker ($fast_linker_var=\"$fast_linker_flags -C link-arg=-fuse-ld=$fast_linker\")"
fi

started=$(date +%s)

if [[ "$runner" == "nextest" ]]; then
    if ((full)); then
        ignored+=(--run-ignored=all)
    fi
    echo "=== cargo nextest run ${scope[*]:-} --all-features ${release[*]:-} ${ignored[*]:-} ${forward[*]:-}"
    # The `${arr[@]+"${arr[@]}"}` form is what bash 3.2 (the `bash` macOS ships) needs to expand
    # an empty array under `set -u`; a bare `"${arr[@]}"` is an unbound-variable error there.
    cargo nextest run ${scope[@]+"${scope[@]}"} --all-features \
        ${release[@]+"${release[@]}"} ${ignored[@]+"${ignored[@]}"} ${forward[@]+"${forward[@]}"}
    # The doctests nextest skips. `--doc` builds rustdoc's tests for every crate and runs the five
    # of them, which is a *whole-workspace* pass: a forwarded filter or package selection is a
    # targeted run, and cargo's `--doc` does not accept the libtest flags the other path forwards,
    # so this step runs only when nothing was forwarded. The line says which of the two happened.
    if (( ${#forward[@]} == 0 )); then
        echo "=== cargo test --doc --workspace --all-features ${release[*]:-}   (nextest runs no doctests)"
        cargo test --doc --workspace --all-features ${release[@]+"${release[@]}"}
    else
        echo "=== doctests skipped: the run was filtered (${forward[*]}), and nextest runs no doctests"
    fi
else
    if ((full)); then
        ignored+=(--include-ignored)
    fi
    echo "=== cargo test ${scope[*]:-} --all-features ${release[*]:-} ${forward[*]:-} -- --nocapture ${ignored[*]:-}"
    cargo test ${scope[@]+"${scope[@]}"} --all-features \
        ${release[@]+"${release[@]}"} ${forward[@]+"${forward[@]}"} \
        -- --nocapture ${ignored[@]+"${ignored[@]}"}
fi

ended=$(date +%s)
echo "=== gate: $runner · $((ended - started))s"
