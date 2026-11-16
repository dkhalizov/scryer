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

Status at the last baseline (2026-11-16, `parity/report/baseline.md`).
Counts include cases captured from our own instances, which stay private.
Green: matches the reference app. Red: differs today. Waived: difference
kept on purpose. Unmapped: catalogued, no test yet. Passing is green out of
green + red.

| Area | Covers | Green | Red | Waived | Unmapped | Passing |
|---|---|---|---|---|---|---|
| parse | release-name parsing | 3216 | 23 | 569 | 117 | 99% |
| decision | which release to grab or reject | 750 | 0 | 167 | 129 | 100% |
| quality | quality ranking, profiles, custom formats | 496 | 0 | 72 | 222 | 100% |
| naming | file and folder names | 490 | 3 | 92 | 148 | 99% |
| import | importing finished downloads | 170 | 0 | 41 | 386 | 100% |
| scan | reading an existing library | 70 | 0 | 66 | 39 | 100% |
| torrent | download client, seeding | 26 | 0 | 24 | 170 | 100% |
| requests | user requests (Seerr) | 13 | 1 | 5 | 320 | 93% |
| maintenance | cleanup rules (Maintainerr) | 39 | 0 | 10 | 365 | 100% |
| subtitles | subtitle matching (Bazarr) | 132 | 0 | 9 | 74 | 100% |
| indexers | indexer settings and routing (Prowlarr) | 32 | 3 | 24 | 3 | 91% |
| notify | notifications | 0 | 0 | 2 | 8 | - |
| flow | end-to-end chain, watchlist to cleanup | 0 | 0 | 0 | 17 | - |
| **all** |  | **5434** | **30** | **1081** | **1998** | **99%** |

By reference app:

| App | Green | Red | Waived | Unmapped | Passing |
|---|---|---|---|---|---|
| Sonarr | 3959 | 22 | 674 | 633 | 99% |
| Radarr | 1267 | 5 | 356 | 530 | 99% |
| Prowlarr | 25 | 2 | 23 | 24 | 93% |
| Bazarr | 133 | 0 | 14 | 81 | 100% |
| Seerr | 11 | 1 | 4 | 315 | 92% |
| Maintainerr | 38 | 0 | 10 | 366 | 100% |
| qBittorrent | 1 | 0 | 0 | 32 | 100% |
| Whole stack | 0 | 0 | 0 | 17 | - |

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
