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
| parse | release-name parsing | 1956 | 1356 | 0 | 613 | 59% |
| decision | which release to grab or reject | 569 | 141 | 0 | 336 | 80% |
| quality | quality ranking, profiles, custom formats | 407 | 39 | 0 | 344 | 91% |
| naming | file and folder names | 67 | 506 | 0 | 160 | 12% |
| import | importing finished downloads | 81 | 21 | 0 | 495 | 79% |
| scan | reading an existing library | 22 | 1 | 0 | 152 | 96% |
| torrent | download client, seeding | 13 | 8 | 20 | 179 | 62% |
| requests | user requests (Seerr) | 4 | 2 | 0 | 333 | 67% |
| maintenance | cleanup rules (Maintainerr) | 16 | 4 | 3 | 391 | 80% |
| subtitles | subtitle matching (Bazarr) | 64 | 63 | 0 | 88 | 50% |
| indexers | indexer settings and routing (Prowlarr) | 4 | 28 | 1 | 29 | 12% |
| notify | notifications | 0 | 0 | 0 | 10 | - |
| **all** | | **3203** | **2169** | **24** | **3130** | **60%** |

By reference app:

| App | Green | Red | Waived | Unmapped | Passing |
|---|---|---|---|---|---|
| Sonarr | 2500 | 1578 | 20 | 1190 | 61% |
| Radarr | 611 | 507 | 1 | 1039 | 55% |
| Prowlarr | 8 | 15 | 0 | 51 | 35% |
| Bazarr | 64 | 63 | 0 | 101 | 50% |
| Seerr | 4 | 2 | 0 | 325 | 67% |
| Maintainerr | 16 | 4 | 3 | 391 | 80% |
| qBittorrent | 0 | 0 | 0 | 33 | - |

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
