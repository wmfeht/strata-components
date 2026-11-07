# S4: syntect regex backend and checkpoint interval

| | |
| --- | --- |
| Status | Proposed. Needs the owner's sign-off, because it reverses plan §4 C10 ("use the `fancy-regex` backend") |
| Work package | `docs/work/phase-0-groundwork/S4-syntect-benchmark.md` |
| Consumers | C10a (highlighter), C10b (code view), C11 (Markdown code blocks) |
| Fixtures | `tests/fixtures/code/` (see its README) |
| Harness | `/home/wmfeht/dev/strata-spikes/s4-bench`, outside the repository and not merged |

## Decision

- **Backend: `onig`.** Decision rule (1) applies. With `fancy-regex` the worst-case viewport
  misses the 8 ms budget on five of the six fixtures at every checkpoint interval measured,
  even 64 lines, and Markdown takes about 190 ms. With `onig` the viewport meets 8 ms on all six
  at N = 64.
- **Checkpoint interval: 64 lines**, not 256. At 256 the `onig` worst-case viewport p95 is
  4.9–14.5 ms. 64 is the largest interval measured that keeps every fixture inside the 8 ms
  budget, and it costs 0.4–1.0 MiB of checkpoints for a 50k-line file.
- **Editable view: cache a state for every line.** A one-word edit then re-highlights in
  0.07 ms p95. With states only at 256-line checkpoints, the same edit takes 6.3 ms p95.

The decision rule was fixed before measuring: choose `fancy-regex` unless (1) it misses a
budget that `onig` meets, (2) its highlighting is visibly wrong on the fixtures, or (3) the
grammars it drops are needed. Rule (2) did not apply: the backends give identical scope ops on
five fixtures, and the Markdown differences come from a different JavaScript grammar (see g),
not from the regex engine. Rule (3) did not need deciding, but `onig` also keeps PowerShell and
the six other grammars that `fancy-regex` drops.

Budgets: viewport highlight ≤ 8 ms on the worker (target 4 ms); edit re-highlight ≤ 2 ms.

## Setup

- syntect 5.3.0 (`parsing` + `regex-fancy` or `regex-onig`, no default features). syntect 5.3
  pins fancy-regex 0.16.2, so the newer fancy-regex releases (0.20) could not be tried.
- two-face 0.5.2+bat-0.26.1, `syntax::extra_newlines()`. Each backend uses its own dump: the
  `fancy` dump has 213 syntaxes and the `onig` dump has 220. Theme: two-face's OneHalfDark.
- Rust 1.99.1 and cargo-deny 0.20.2 (the repository's pins). Release profile, `lto = true`,
  `codegen-units = 1`.
- Machine: Intel Core Ultra 9 285K, Linux 7.2. The benchmark is single-threaded and pinned to
  P-core 7 with `taskset`. The governor was `powersave`, and the machine was shared with other
  builds (load average 4.4–8.3 on 24 cores). Builds ran on E-cores with `CARGO_BUILD_JOBS=6`.
  The backends alternated A B A B, and the two rounds agree within about 2 %. Read the ratios
  as the headline: absolute times depend on this machine.
- JavaScript is forced to the `JavaScript` grammar on both backends. By extension, `onig`
  picks `JavaScript (Babel)`, which `fancy-regex` lacks, and comparing two grammars would
  measure the grammar rather than the backend. Babel's own cost on `onig` is listed under c.
- "Viewport" means 80 visible lines plus 40 lines of overscan. The worst case resumes from a
  checkpoint N − 1 lines above the viewport, so it parses and highlights N − 1 + 120 lines.

## Results

Times are in ms unless marked. "fancy / onig" pairs give medians, with p95 in brackets where
it matters.

### a, b: cold start (30 fresh processes each)

| | fancy | onig | ratio |
| --- | --- | --- | --- |
| Load `extra_newlines()` | 1.59 (p95 3.00) | 2.08 (p95 3.06) | 0.77× |
| First colours, Rust (load, detect, lines 0–255, regex compile) | 37.7 | 6.9 | 5.5× |
| First colours, C | 29.4 | 5.9 | 5.0× |
| First colours, JavaScript | 101.9 | 15.4 | 6.6× |
| First colours, Python | 46.8 | 8.9 | 5.2× |
| First colours, Markdown | 254.1 | 14.1 | 18.0× |
| First colours, shell | 38.7 | 6.6 | 5.9× |
| Compile every pattern in the set (h) | 3,437 | 200 | 17× |

The set loads fast on both backends because syntect loads contexts and compiles regexes
lazily. The cold cost is regex compilation, and there `fancy-regex` is 5–18× slower.

### c: full parse, warm (1 warm-up and 10 runs, both rounds)

| Fixture | Parse only, fancy / onig | Parse + highlight, fancy / onig | ratio |
| --- | --- | --- | --- |
| rust-50k | 2.20 s / 0.75 s | 2.30 s / 0.84 s | 2.7× |
| c-50k | 1.49 s / 0.54 s | 1.55 s / 0.59 s | 2.7× |
| js-50k | 2.66 s / 0.85 s | 2.83 s / 0.99 s | 2.9× |
| python-50k | 2.83 s / 1.02 s | 2.92 s / 1.10 s | 2.7× |
| markdown-50k | 29.2 s / 1.28 s | 29.3 s / 1.37 s | 21× |
| shell-5k | 193 / 64 | 204 / 74 | 2.8× |

With `onig`'s own detection, js-50k uses `JavaScript (Babel)`, and parse plus highlight takes
1.27 s (28 % more than `JavaScript`).

### d: 256-line window resumed from a 256-line checkpoint (200 seeded positions)

| Fixture | fancy | onig | ratio |
| --- | --- | --- | --- |
| rust-50k | 17.9 (23.1) | 6.5 (8.5) | 2.7× |
| c-50k | 12.1 (16.0) | 4.7 (6.1) | 2.6× |
| js-50k | 21.8 (30.3) | 8.0 (10.9) | 2.8× |
| python-50k | 22.0 (29.6) | 8.3 (11.1) | 2.6× |
| markdown-50k | 232 (361) | 11.3 (16.1) | 21× |
| shell-5k | 15.0 (20.8) | 5.6 (7.5) | 2.7× |

On both backends, all 200 windows in every fixture gave exactly the spans of a full parse
(0 mismatches), so resuming from a cloned `ParseState` + `HighlightState` is exact.

### e: checkpoint interval sweep, worst-case viewport p95

| N (lines parsed) | rust | c | js | python | markdown | shell |
| --- | --- | --- | --- | --- | --- | --- |
| 64 (183), onig | **3.8** | **2.6** | **4.3** | **4.5** | **7.8** | **3.1** |
| 64, fancy | 10.0 | 6.5 | 12.0 | 11.8 | 189 | 9.0 |
| 128 (247), onig | 4.9 | 3.3 | 6.5 | 6.3 | 9.5 | 4.2 |
| 128, fancy | 12.8 | 8.8 | 18.2 | 16.0 | 233 | 11.7 |
| 256 (375), onig | 7.2 | 4.9 | 9.0 | 9.3 | 14.5 | 6.2 |
| 256, fancy | 18.9 | 12.9 | 25.2 | 23.5 | 334 | 16.7 |
| 512 (631), onig | 11.5 | 8.1 | 14.1 | 15.0 | 22.1 | 10.1 |
| 1024 (1143), onig | 20.5 | 14.3 | 25.1 | 26.8 | 37.0 | 18.1 |

Checkpoint memory (deep size of the cloned states, counting allocator, `onig`; `fancy` is
within 20 %):

| N | rust | c | js | python | markdown | shell-5k |
| --- | --- | --- | --- | --- | --- | --- |
| 64 | 792 KiB | 502 KiB | 1001 KiB | 381 KiB | 795 KiB | 59 KiB |
| 256 | 199 KiB | 126 KiB | 254 KiB | 95 KiB | 193 KiB | 14 KiB |
| Every line | 50 MiB | 32 MiB | 63 MiB | 24 MiB | 51 MiB | 3.5 MiB |

Keeping checkpoints adds 0–3 % to a full pass for source fixtures and 3–8 % for Markdown, at
every N.

### f: re-highlight after a one-line edit (shell-5k)

| Edit | Per-line cache, until the state matches | Per-line cache, capped at 120 lines | Checkpoints only (N = 256) |
| --- | --- | --- | --- |
| 200 word renames, onig | 0.025 (0.076) | 0.023 (0.072) | 3.8 (6.3) |
| 200 word renames, fancy | 0.046 (0.51) | 0.042 (0.50) | 10.4 (12.9) |
| Unmatched `"` at 5 lines, onig | 6.0 (24.2) | 0.78 (0.92) | 8.5 (25.0) |
| Unmatched `"`, fancy | 7.3 (29.3) | 0.93 (1.13) | 12.9 (36.2) |
| `cat <<EOF` at 5 lines, onig | 0.13 (0.24) | 0.13 (0.24) | 3.8 (8.7) |
| `cat <<EOF`, fancy | 0.38 (2.06) | 0.37 (2.02) | 10.9 (19.0) |
| `case "$x" in` at 5 lines, onig | 0.24 (0.81) | 0.23 (0.81) | 6.9 (8.6) |
| `case "$x" in`, fancy | 0.29 (1.90) | 0.28 (1.88) | 11.5 (19.6) |

A rename re-parses 1 line (median), and 4 lines at p95. An unmatched quote re-parses every
line to the end of the file (up to 4,960), and a heredoc or `case` 12–78 lines. `fancy-regex`'s
p95 for heredocs and `case` comes close to 2 ms because syntect compiles a fresh regex for each
back-referencing end pattern (the heredoc's `EOF`), and that compile is slow in `fancy-regex`.

### g: correctness diff of scope ops

Identical on every line of rust, c, js, python and shell. Markdown differs on 5,789 lines, and
all of them lie in or on the fences of its ` ```js ` blocks. The fences embed `source.js`, which
resolves to `JavaScript (Babel)` in the `onig` set and to `JavaScript` in the `fancy` set. The
grammar differs, not the regex engine.

### h: grammar sweep (`Regex::try_compile` over every match pattern)

`onig` compiles all 22,257 patterns of the full 220-syntax set. `fancy-regex` fails on
patterns in 7 syntaxes, which is exactly the set two-face leaves out of its `fancy` dump:
ARM Assembly, Hosts File, JavaScript (Babel), PowerShell, Regular Expressions (Elixir),
Salt State (SLS) and VimHelp. The failures are subroutine calls, a variable-length
look-behind, an unknown escape, and pattern parse and NFA build errors.

### i: one very long JavaScript line (warm, 5 runs)

| Line | fancy | onig |
| --- | --- | --- |
| 100 KB array literal | 184 | 64 |
| 400 KB array literal | 537 | 275 |
| 100 KB of joined statements ("minified") | 366 | 181 |
| 400 KB of joined statements | 1,737 | 1,217 |

Cost grows faster than length: 4× the minified text costs 6.7× the time on `onig`.

### j: binary size and build

| | fancy | onig |
| --- | --- | --- |
| Size added (`size_probe` − `size_base`) | 3.10 MB, 2.85 MB stripped | 1.79 MB, 1.71 MB stripped |
| Crates (normal + build) | 29 | 30 |
| Clean release build of the probe (LTO, 6 jobs) | 14.1–14.4 s | 11.8–12.4 s |
| Clean dev build | 3.8 s | 4.4 s |

The probe links two-face's syntax set, parsing and highlighting, with a theme built in code
(K1 builds themes, so two-face's theme dump is left out). `onig` comes out 1.1 MB smaller:
`fancy-regex` brings regex-automata and its Unicode tables, while `onig` links Oniguruma
statically, built from bundled C. `onig`'s extra crates are build-time only (`cc`,
`pkg-config`, `shlex`, `find-msvc-tools`).

### k: cargo deny (the repository's `deny.toml`, graph narrowed to each backend)

- `cargo deny check licenses`: **ok** for `fancy`, `onig`, and both together.
- `cargo deny check`: bans, licenses and sources ok. Advisories **fail** on both backends with
  one finding: `bincode 1.3.3`, **RUSTSEC-2025-0141** (unmaintained; not a vulnerability). Both
  syntect (`parsing` turns on `dump-load`/`dump-create`) and two-face need it to read the
  syntax dumps, so no feature choice avoids it.
- `onig_sys` declares `MIT`, but it compiles the bundled Oniguruma C sources, which are
  BSD-2-Clause (allowed). cargo-deny sees only the crate's declared licence.
- The yaml-rust advisory is not pulled in: the lines below leave out `yaml-load`, which comes
  with `default-onig`.

### l: syntect's own default syntaxes

Every one of syntect's 75 default syntaxes is in two-face's set, by name or scope, on both
backends. C10a doesn't need syntect's `default-syntaxes` feature.

## For C10a

**Dependency lines** for `[workspace.dependencies]` (avoid `default-onig`/`default-fancy`,
which add html, plist, yaml-load and the default themes):

```toml
# Regex backend and grammar set chosen in S4 (docs/decisions/S4-syntect-backend.md).
syntect = { version = "5.3.0", default-features = false, features = ["parsing", "regex-onig"] }
two-face = { version = "0.5.2", default-features = false, features = ["syntect-onig"] }
```

The two must use the same backend: two-face embeds a dump per backend.

**Advisory ignore** C10a will need. It is for the owner to approve, since AGENTS.md asks for a
stated reason:

```toml
{ id = "RUSTSEC-2025-0141", reason = "bincode 1 is syntect's and two-face's syntax dump format; unmaintained, no known vulnerability, and its authors call 1.3.3 complete" },
```

**Build requirement:** `onig_sys` needs a C compiler at build time. It does not need bindgen
or libclang, because syntect turns off onig's `generate` feature. To link a distribution's
Oniguruma instead, set `RUSTONIG_SYSTEM_LIBONIG=1` (pkg-config); that is Strata's packaging
call.

**Syntax set:** `two_face::syntax::extra_newlines()`. The measurements used the `newlines`
set, so feed lines with their `\n`, and add one to a final line that has none.

**Grammars:** with `onig`, none are missing: all 220 load. For the record, `fancy-regex` would
have lost the seven listed under h.

**Notice duty:**

- `two_face::acknowledgement::listing()` holds the 66 syntax licence files that need acknowledgement
  (50 MIT, 13 Apache-2.0, 3 BSD-2/3-Clause), plus the bat curation note.
  Strata's credits must carry them, either the `to_md()` text (about 11 KB) or the full list at
  `two_face::acknowledgement::url()`.
- Oniguruma's BSD-2-Clause copyright (`onig_sys/oniguruma/COPYING`) needs the same treatment.
- Some embedded grammars carry licences that cargo-deny never sees and that the allow list does
  not name: the GraphQL grammar is WTFPL; GLSL and PureScript are Unlicense; Sublime's default
  packages use Sublime's permissive licence. None of them requires a notice.

**Jump to end:** a warm full parse and highlight of a 50k-line file takes 0.59–1.37 s on
`onig` (on this machine). Checkpoints for the end of a file only exist once a pass has reached
it. So build them in one background pass right after open, serve each viewport from the
nearest checkpoint built so far, and draw lines plain until the pass reaches them. The first
colours at the top of a file arrive in 6–15 ms cold.

**Long lines:** skip highlighting any line longer than **16 KiB**: draw it plain and carry the
parse state over unchanged. A 100 KB line costs 64–181 ms on `onig`, and cost grows faster than
length. Rune-kit's built-in highlighter has a similar guard (it colours the first 32 KiB of a
block). C10a's perf test should cover one such line.

**Edits:** keep a state per line in the editable variant (scripts are small: 3.5 MiB for
5,000 lines), and re-highlight synchronously only up to the viewport plus overscan. Finish an
unconverged cascade, such as an unmatched quote, on the worker: a cascade to the end of the
file took up to 24 ms.

## Not measured

- Real-world corpora. The fixtures are synthetic, generated from templates, so token mix and
  line lengths are plausible but not sampled from real projects.
- Why Markdown is 21× slower on `fancy-regex`. Irrelevant now that `onig` is chosen.
- Checkpoint intervals below 64. The viewport itself (120 lines) puts a floor on the cost, and
  Markdown stays above the 4 ms target at 64.
- Checkpoint-only edit caching at N = 64. By estimate it is about 1 ms; it was measured at 256
  only.
- K1's real themes. Highlight cost grows with the number of theme selectors; this used
  OneHalfDark.
- The resident memory of a loaded syntax set, and contention when several workers share it.
- Linking against a system Oniguruma, other CPUs, and the UI-thread commit budget (C10b's
  `RUNE_COMMIT_BUDGET_MS` runs).
- Behaviour with CRLF input, the `no-newlines` set, and first-line detection.
