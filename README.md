# ojrs

A Rust port of [online-judge-tools/oj](https://github.com/online-judge-tools/oj).

The goal is CLI-level compatibility with upstream: flag names, defaults, and console output should match the original `oj` so existing scripts keep working.

## Migration status

### Subcommands

Listed in planned migration order.

| upstream `oj` | ojrs | Status |
| --- | --- | --- |
| `test` (`t`) | ✅ | Done |
| `download` (`d`, `dl`) | 🚧 | AtCoder only; `--system` not yet wired |
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

`test` runs locally, so it's judge-agnostic. `download`, `login`, and `submit` currently support AtCoder only; other services return `unsupported judge`.

| Judge | `download` | `login` | `submit` |
| --- | --- | --- | --- |
| AtCoder | ✅ | 🚧 | 🚧 |
| Codeforces | ⏳ | ⏳ | ⏳ |
| yukicoder | ⏳ | ⏳ | ⏳ |
| Aizu Online Judge | ⏳ | ⏳ | ⏳ |
| others | ⏳ | ⏳ | ⏳ |

## Build

```
cargo build --release
```

The binary is `target/release/oj` (named `oj` to match upstream).

## Usage

```
oj download https://atcoder.jp/contests/abc100/tasks/abc100_a
oj download -n https://atcoder.jp/contests/abc100/tasks/abc100_a   # dry-run

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
