# oj-rs

A Rust port of [online-judge-tools/oj](https://github.com/online-judge-tools/oj).

The goal is CLI-level compatibility with upstream: flag names, defaults, and console output should match the original `oj` so existing scripts keep working.

## Migration status

See [MIGRATION.md](MIGRATION.md) for the per-subcommand and per-flag porting status, plus per-judge support details.

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
