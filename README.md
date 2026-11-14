# Scryer, personal fork

This is an independent fork of [Scryer](https://github.com/scryer-media/scryer),
kept to replace an *arr stack: Sonarr, Radarr, Prowlarr, Bazarr, Seerr and
Maintainerr. qBittorrent stays as the download client. It runs in a home
media stack on Kubernetes.

This is not the upstream project. It is for personal use. Upstream is a source of
fixes; a change goes back only now and then, as a separate PR. For the original README, docs, releases and community,
see [scryer-media/scryer](https://github.com/scryer-media/scryer).

All credit for Scryer goes to its upstream authors. This fork adds patches and
a test net on top of their code.

## What differs from upstream

- Patches for gaps that block replacing the *arr tools. Examples: scanned files
  get a quality, torrent stall recovery, subtitle matching like Bazarr, and
  release-name parser fixes.
- A parity net: tests that compare our behaviour with the apps we replace.
- Working notes and a decision log, kept local (gitignored).
- No schema migrations of our own, so upstream migrations keep applying.

## Parity net

Each behaviour of the replaced apps is a catalogue entry with an expected
answer. A test runs it against our code. A green case that fails breaks the
run, and a red case that starts passing must be marked green. This keeps
regressions out while we fix gaps. See [`parity/README.md`](parity/README.md).

```sh
parity/run.sh
```

Status at the last baseline (2026-11-14, `parity/report/baseline.md`).
Counts include cases captured from our own instances, which stay private.
Green: matches the reference app. Red: differs today. Waived: difference
kept on purpose. Unmapped: catalogued, no test yet. Passing is green out of
green + red.

| Area | Covers | Green | Red | Waived | Unmapped | Passing |
|---|---|---|---|---|---|---|
| parse | release-name parsing | 2224 | 1488 | 96 | 117 | 60% |
| decision | which release to grab or reject | 644 | 193 | 80 | 129 | 77% |
| quality | quality ranking, profiles, custom formats | 470 | 93 | 5 | 222 | 83% |
| naming | file and folder names | 424 | 158 | 3 | 148 | 73% |
| import | importing finished downloads | 151 | 44 | 16 | 386 | 77% |
| scan | reading an existing library | 70 | 34 | 32 | 39 | 67% |
| torrent | download client, seeding | 25 | 1 | 24 | 170 | 96% |
| requests | user requests (Seerr) | 12 | 3 | 4 | 320 | 80% |
| maintenance | cleanup rules (Maintainerr) | 37 | 5 | 7 | 365 | 88% |
| subtitles | subtitle matching (Bazarr) | 123 | 11 | 7 | 74 | 92% |
| indexers | indexer settings and routing (Prowlarr) | 7 | 28 | 24 | 3 | 20% |
| notify | notifications | 0 | 0 | 2 | 8 | - |
| **all** | | **4187** | **2058** | **300** | **1981** | **67%** |

By reference app:

| App | Green | Red | Waived | Unmapped | Passing |
|---|---|---|---|---|---|
| Sonarr | 3148 | 1373 | 134 | 633 | 70% |
| Radarr | 852 | 655 | 121 | 530 | 57% |
| Prowlarr | 16 | 11 | 23 | 24 | 59% |
| Bazarr | 124 | 11 | 12 | 81 | 92% |
| Seerr | 10 | 3 | 3 | 315 | 77% |
| Maintainerr | 36 | 5 | 7 | 366 | 88% |
| qBittorrent | 1 | 0 | 0 | 32 | 100% |

## Build and test

The Rust toolchain is pinned in `rust-toolchain.toml`. Run focused tests per
crate, not the whole workspace:

```sh
cargo fmt --all
cargo nextest run --locked -p <crate> <test-filter>
```

Postgres tests skip unless `SCRYER_TEST_POSTGRES_URL` is set. Never run the
app against a real library without a deliberate decision to do so.

## Branches and upstream sync

`own/main` is the trunk. `main` mirrors the last upstream release we took.
Upstream releases and single fixes are merged in when we want them.
Working notes, data captured from our own instances and the capture tools
are gitignored and stay local.

Contributor and agent rules for this fork are in [`CLAUDE.md`](CLAUDE.md).

## License

GPL-3.0, as upstream. See [`LICENSE`](LICENSE).
