# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project goal

Rust port of [online-judge-tools/oj](https://github.com/online-judge-tools/oj). The original is a Python CLI for competitive programming workflows. This repo aims for **CLI-level compatibility** with upstream — flags, defaults, exit codes, and output text should match where reasonable. Diverge only when clap or Rust idioms force it; document the divergence in code if non-obvious.

## Naming convention

The maintainer is a native Japanese speaker. **Use natural English** for new identifiers (variables, functions, types, fields), and update direct-translation names in passing when editing. Function names should read as actions (`build_pair_regex`, `kill_process_tree`), not nouns.

| Avoid | Prefer | Reason |
|---|---|---|
| `start_time` | `started_at` | Timestamp semantics; pair with `elapsed: Duration` |
| `ac_count` | `accepted_count` | Non-domain code spells out |
| `result` (variable name) | `outcome` / `verdict` | Avoid clash with `Result<T, E>` |
| `param_for_X`, `data_of_X` | `request` / `response`, `input` / `output`, `query` / `result` | Natural English pairs |
| `Verdict::Ac` / `Wa` / `Re` / `Tle` | `Verdict::Accepted` / `WrongAnswer` / `RuntimeError` / `TimeLimitExceeded` | Enum variants spell out the full word |

The short forms `AC` / `WA` / `RE` / `TLE` are universal CP terms — keep them as user-facing string labels (returned by `Verdict::label()`) for upstream compat.

Keep the upstream-facing surface (CLI flag names, value-enum strings, format placeholders like `%s.%e`, verdict labels, and console output text) unchanged — those are the compat contract. `ConsoleReporter` mirrors upstream's `[*]`/`[+]`/`[-]` line prefixes, `slowest:` line, and `test success/failed: …` summary verbatim.

## Common commands

```bash
cargo build                  # debug build → target/debug/oj
cargo build --release        # release build (LTO enabled in Cargo.toml)
cargo test                   # all unit tests
cargo test domain::compare:: # run a single module's tests
cargo run -- test -c ./a.out -d test/  # invoke the oj binary via cargo
```

The binary is named `oj` (not `oj-rs`) to match upstream's command name — see `[[bin]]` in `Cargo.toml`.

## Commit messages

Follow [Angular Conventional Commits](https://github.com/angular/angular/blob/main/contributing-docs/commit-message-guidelines.md). Types: `build`, `ci`, `docs`, `feat`, `fix`, `perf`, `refactor`, `test`, `chore`.

## Architecture

Clean Architecture / hexagonal: dependencies point inward only — `main → cli + infrastructure → application → domain`. Inner layers never import outer layers. The `cli` layer owns clap and converts clap-derived types into use-case inputs via `From` so `domain` and `application` stay framework-free. `main.rs` is the composition root.

When adding a new subcommand, follow the existing `test` subcommand as a template, and mirror upstream `oj <name>` for flag names, defaults, and value-enum strings.
