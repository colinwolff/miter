//! `miter` binary entry point.
//!
//! All logic lives in the library ([`miter::cli`]) so that the subcommands are covered by
//! the crate's tests; this shim only maps the result to a process exit code.

fn main() {
    std::process::exit(miter::cli::run());
}
