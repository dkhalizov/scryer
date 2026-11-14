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
| parse | release-name parsing | 2224 | 1584 | 0 | 117 | 58% |
| decision | which release to grab or reject | 644 | 272 | 0 | 130 | 70% |
| quality | quality ranking, profiles, custom formats | 470 | 98 | 0 | 222 | 83% |
| naming | file and folder names | 424 | 161 | 0 | 148 | 72% |
| import | importing finished downloads | 151 | 60 | 0 | 386 | 72% |
| scan | reading an existing library | 70 | 66 | 0 | 39 | 51% |
| torrent | download client, seeding | 17 | 9 | 20 | 174 | 65% |
| requests | user requests (Seerr) | 10 | 9 | 0 | 320 | 53% |
| maintenance | cleanup rules (Maintainerr) | 37 | 6 | 3 | 368 | 86% |
| subtitles | subtitle matching (Bazarr) | 65 | 76 | 0 | 74 | 46% |
| indexers | indexer settings and routing (Prowlarr) | 7 | 29 | 1 | 25 | 19% |
| notify | notifications | 0 | 0 | 0 | 10 | - |
| **all** | | **4119** | **2370** | **24** | **2013** | **63%** |

By reference app:

| App | Green | Red | Waived | Unmapped | Passing |
|---|---|---|---|---|---|
| Sonarr | 3144 | 1489 | 20 | 635 | 68% |
| Radarr | 852 | 772 | 1 | 533 | 52% |
| Prowlarr | 12 | 16 | 0 | 46 | 43% |
| Bazarr | 66 | 79 | 0 | 83 | 46% |
| Seerr | 8 | 8 | 0 | 315 | 50% |
| Maintainerr | 36 | 6 | 3 | 369 | 86% |
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
