# Parity net

The parity net is a test-only check of our Scryer fork against the apps it
replaces (Sonarr, Radarr, Prowlarr, Bazarr, Seerr, Maintainerr, qBittorrent).
Each behaviour is a catalogue entry with an expected answer taken from the
reference apps. Green means our code gives that answer. Red means it gives a
different one today, and the net remembers that, so a regression in a green
case fails the run. Design: `notes/11-parity-net-design.md`. Log:
`notes/12-parity-net-progress.md`.

## Run it

```sh
parity/run.sh              # validate, run every suite, write parity/report/latest.md
parity/run.sh --baseline   # same, then freeze it as report/baseline.{json,md}
```

The script runs the harness tests, then one focused `cargo nextest` run per
crate for the suites the catalogue names, then writes the report. It exits 1
on a ratchet violation, a catalogue error, or a suite that did not run.

Read `parity/report/latest.md`. The table counts entries per area by status.
"Run" gives suites run, cases recorded and violation counts. "Moved since
baseline" lists every ID whose status changed since `report/baseline.*`.
Per-suite results land in `parity/report/results/`.

## Statuses

Every entry has one status in its catalogue file.

| Status | Meaning |
|---|---|
| unmapped | catalogued, no harness test yet (`tests: []`) |
| red | a test runs it and it fails today |
| green | a test runs it and it passes |
| waived | not wanted; `decision` says why (ledger reference) |

The ratchet is the rule that the outcome must match the status. A green case
that fails breaks the run. A red case that passes also breaks it, until you
flip it to green in the catalogue; that flip is the record of the fix.

Entries with `in_scope: false` stay `unmapped` or `waived`. A waived entry
needs a `decision`. Each entry's `source` is `reference`, `captured`, `config`
or `observed`.

## Add or map a case

IDs are `<APP>-<AREA>-<nnnn>`. APP is one of SON RAD PRW BAZ SER MNT QBT.
AREA is the catalogue file's area in capitals (`parse.sonarr.yaml` has area
`parse`, so `SON-PARSE-0123`). The number has at least four digits. IDs are
never reused or renumbered.

1. Add the corpus case (with its `id`) under
   `corpus/reference/<app>/<area>/` and the catalogue entry in
   `catalogue/<area>[.<part>].yaml`.
2. In the harness test, record one outcome per ID:

```rust
let mut suite = scryer_parity::Suite::new("scryer-application::parity_parse::son_parse");
for case in cases { suite.check(&case.id, case.expected, actual(&case)); }
suite.finish(); // panics on: green that fails, red that passes, an ID not run
```

   The suite name is `<crate>::parity_<name>::<fn>`. Skip cases the suite does
   not want with `if !suite.wants(&case.id) { continue }`.
3. First mapping of a suite: run it with `PARITY_ADOPT=1`, then
   `python3 -I parity/adopt.py parity/report/results/<suite>.json` writes
   `tests:` and the first status (pass is green, fail is red) for its
   `unmapped` IDs. Until that suite's first commit,
   `python3 -I parity/adopt.py --reset <suite>` puts them back to unmapped, for
   example after a mapping fix. After the commit the ratchet owns them.
4. Run `parity/run.sh`.

Never edit an expected answer to make a case pass. If the reference is wrong
for us, set the entry to `waived` with a decision in `notes/10-patch-ledger.md`.

## Change product code

A commit that touches product code (`crates/*/src`, `build.rs`, web app
source) must carry a trailer. `.githooks/commit-msg` refuses it otherwise:

```
Parity: SON-PARSE-0123, RAD-DECISION-6002   # IDs moved; stage a test, catalogue or corpus change
Parity: none (refactor, no behaviour a parity ID covers)
```

The IDs must exist in `parity/catalogue`. Merge commits are exempt.

Before committing, check that the tests would notice your change being
broken. `cargo mutants` mutates only the lines you changed:

```sh
git diff own/main > /tmp/change.diff   # or: git diff HEAD
cargo mutants --in-diff /tmp/change.diff -p <crate> --test-tool nextest \
  -- --test parity_<area>              # the suites that cover the change
```

Leave out `--lib` for `scryer-application`: its unit-test suite rebuilds and
runs for about 25 minutes per mutant.

A `MISSED` mutant in your lines means no test pins that behaviour. Add one.

Coverage is a map, never a gate. Use it to see where nothing is tested:

```sh
cargo llvm-cov nextest -p <crate> --summary-only
```

Baselines for both, `report/mutants-baseline.md` and
`report/coverage-baseline.md`, are the phase 6 output. They are not committed
yet (`.gitignore` already lets them through).

## Properties, proofs, fuzzing

These check behaviour beyond the catalogue's examples.

- Property tests: `crates/*/tests/props_*.rs` and
  `crates/scryer-application/tests/sm_download_import.rs` (a state machine).
  They run with the normal crate tests.
- Kani proofs: `crates/scryer-application/src/kani_proofs.rs`, run with
  `cargo kani -p scryer-application -Z stubbing`. The file header lists which
  proofs verify, which time out, and the extra flags.
- Fuzz targets: `parity/fuzz/` (naming, scan_path). Parser targets live in
  `crates/scryer-release-parser/fuzz`. The toolchain and run commands are in
  the comment at the top of `parity/fuzz/Cargo.toml`.

Fuzz corpora (`parity/fuzz/corpus/`) are local only and gitignored. Seed them
from the release names in `corpus/reference/` only, never from
`corpus/captured/`. Crashes go to `parity/fuzz/artifacts/<target>/`. A crash
is a finding: keep the minimized input and report it.

## Layout

- `catalogue/<area>[.<part>].yaml`: behaviour entries. Area is the file name
  before the first dot. Areas: parse, decision, quality, naming, import, scan,
  torrent, requests, maintenance, subtitles, indexers, notify. The validator
  is `scryer_parity::Catalogue::validate`; `parity-report validate` runs it.
- `corpus/reference/<app>/<area>/*.json`: expected answers ported from the
  reference test suites.
- `corpus/captured/`: frozen answers read from our own instances. Private:
  gitignored, kept on disk only (ledger D45), and never copied elsewhere (no
  fuzz corpus, no other folder). Suites whose entries are all private skip
  when it is absent.
- `extract/`: scripts that build the reference corpus. `capture/`: scripts
  that read our instances for the captured corpus.
- `harness/`: crate `scryer-parity` (test-only): catalogue, ratchet, report,
  and the `parity-report` binary.
- `fuzz/`: standalone fuzz crate, outside the root workspace.
- `adopt.py`: first statuses for newly mapped suites.
- `run.sh`: the one-command run.
- `report/`: generated. Only `baseline.json` and `baseline.md` are committed.
