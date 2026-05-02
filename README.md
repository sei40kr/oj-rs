# oj-rs

A Rust port of [online-judge-tools/oj](https://github.com/online-judge-tools/oj).

The goal is CLI-level compatibility with upstream: flag names, defaults, and console output should match the original `oj` so existing scripts keep working.

## Migration status

### Subcommands

Listed in planned migration order.

| upstream `oj` | oj-rs | Status |
| --- | --- | --- |
| `test` (`t`) | ✅ | Done |
| `download` (`d`, `dl`) | 🚧 | AtCoder + Library Checker + Aizu Online Judge + HackerRank + CS Academy (samples); `--system` not yet wired |
| `login` | 🚧 | AtCoder only; form login blocked by Cloudflare Turnstile (use `--cookie`) |
| `submit` (`s`) | 🚧 | AtCoder only; POST blocked by Cloudflare Turnstile (same as `login`) |
| `generate-input` (`g/i`) | ⏳ | Not started |
| `generate-output` (`g/o`) | ⏳ | Not started |
| `test-reactive` (`t/r`) | ⏳ | Not started |

### `oj test` flags

| Flag | Status |
| --- | --- |
| `-c, --command` | ✅ |
| `-d, --directory` | ✅ |
| `-f, --format` | ✅ |
| `-m, --compare-mode` | ✅ |
| `-M, --display-mode` | ✅ |
| `-S, --ignore-spaces` | ✅ |
| `-N, --ignore-spaces-and-newlines` | ✅ |
| `-D, --diff` | ✅ |
| `-s, --silent` | ✅ |
| `-e, --error` | ✅ |
| `-t, --tle` | ✅ |
| `-j, --jobs` | ✅ |
| `-i, --print-input` | ✅ |
| `--ignore-backup` | ✅ |
| `--judge-command` | ✅ |
| Positional args | ✅ |
| `--mle` | ⏳ |
| `--gnu-time` | ⏳ |
| `--print-memory` | ⏳ |
| `--log-file` | ⏳ |

### `oj login` flags

| Flag | Status |
| --- | --- |
| `-u, --username` | ✅ |
| `-p, --password` | ✅ |
| `--check` | ✅ |
| `--cookie` | ✅ |
| Positional URL | ✅ |
| `--use-browser` | ⏳ |

### `oj download` flags

| Flag | Status |
| --- | --- |
| `-d, --directory` | ✅ |
| `-f, --format` | ✅ |
| `-n, --dry-run` | ✅ |
| `-s, --silent` | ✅ |
| Positional URL | ✅ |
| `-a, --system` | ⏳ |
| `--yukicoder-token` | ⏳ |
| `--dropbox-token` | ⏳ |
| `--log-file` | ⏳ |

### `oj submit` flags

| Flag | Status |
| --- | --- |
| `-l, --language` | ✅ |
| `--no-guess` | ✅ |
| `-y, --yes` | ✅ |
| `-w, --wait` | ✅ |
| `--cookie` | ✅ |
| Positional URL + FILE | ✅ (URL required; history-based guess not ported) |
| `--guess-cxx-latest` | ⏳ |
| `--guess-cxx-compiler` | ⏳ |
| `--guess-python-version` | ⏳ |
| `--guess-python-interpreter` | ⏳ |
| `--open` / `--no-open` | ⏳ (submission URL is printed instead) |

### Supported online judges

`test` runs locally, so it's judge-agnostic. `download` supports AtCoder, Library Checker, Aizu Online Judge, HackerRank, and CS Academy; `login` and `submit` are AtCoder-only. Other services return `unsupported judge`.

| Judge | `download` | `login` | `submit` |
| --- | --- | --- | --- |
| AtCoder | ✅ | 🚧 | 🚧 |
| Library Checker | ✅ (samples) | ❌ Firebase Auth | ❌ Firebase Auth |
| Aizu Online Judge | ✅ (samples) | ❌ JSON session API | ❌ JSON session API |
| HackerRank | ✅ (samples) | ❌ JS-driven CSRF | ❌ JS-driven CSRF |
| CS Academy | ✅ (samples) | ❌ upstream parity | ❌ upstream parity |
| Codeforces | ⏳ | ⏳ | ⏳ |
| yukicoder | ⏳ | ⏳ | ⏳ |
| others | ⏳ | ⏳ | ⏳ |

Library Checker samples come from the official frontend's data sources (`v3.api.judge.yosupo.jp` for the testcases hash, then the public GCS bucket), not by cloning `yosupo06/library-checker-problems` and running `generate.py` like upstream Python `oj`. As a consequence, only example cases are downloadable; `--system` will not be supported via this path.

Aizu Online Judge samples come from `judgedat.u-aizu.ac.jp/testcases/samples/{id}`, with a fallback to scraping the description HTML at `judgeapi.u-aizu.ac.jp/resources/descriptions/ja/{id}` for problems whose samples are only embedded in the statement.

HackerRank samples are scraped from `model.body_html` on the public REST endpoint (`/rest/contests/{contest}/challenges/{slug}`), not from the `download_testcases` ZIP that upstream Python `oj` uses. Only the in-statement samples are available; system tests served by the ZIP endpoint require a logged-in session and are out of scope until `--system` is wired up.

CS Academy is a SPA, so samples are pulled from the same internal JSON endpoints the official frontend uses (`/contest/<name>/` for the task list and `/contest/get_contest_task/` for the example tests), gated by the `csrftoken` cookie set by the homepage.

## Build

```
cargo build --release
```

The binary is `target/release/oj` (named `oj` to match upstream).

## Usage

```
oj download https://atcoder.jp/contests/abc100/tasks/abc100_a
oj download -n https://atcoder.jp/contests/abc100/tasks/abc100_a   # dry-run
oj download https://judge.yosupo.jp/problem/aplusb                 # Library Checker
oj download https://onlinejudge.u-aizu.ac.jp/courses/lesson/2/ITP1/1/ITP1_1_A  # Aizu Online Judge
oj download https://www.hackerrank.com/challenges/solve-me-first   # HackerRank
oj download https://csacademy.com/contest/round-38/task/path-union/  # CS Academy

oj test -c ./a.out -d test/
oj test -c "python3 solution.py" -t 2 -e 1e-6 -N
oj test -c ./a.out -j 4 -D

oj login https://atcoder.jp/                     # interactive prompt
oj login --check https://atcoder.jp/             # is the saved session still valid?
oj login --cookie ./cookies.json https://atcoder.jp/  # reuse a browser-exported jar

oj submit https://atcoder.jp/contests/abc100/tasks/abc100_a main.cpp
oj submit -l "C++ 23" -y https://atcoder.jp/contests/abc100/tasks/abc100_a main.cpp
```

## License

MIT
