# Fork guide for agents

This checkout is our **independent fork** of Scryer, used to replace our *arr
stack. We took it to avoid writing a Rust replacement from scratch, not to
contribute upstream. `own/main` is the trunk: upstream code + our changes.
`notes/` and private parity data are local only (gitignored, ledger D45). Upstream is a source of fixes we may pull, nothing more.

## Before any work

1. Read `notes/README.md`, then `notes/07-personal-fork.md` (the plan) and
   `notes/10-patch-ledger.md` (what is in flight and why).
   For parity-net work: `notes/11-parity-net-design.md` and
   `notes/12-parity-net-progress.md` (the next step is at the top).
2. **Docs before code.** A code change starts with a ledger entry (problem,
   cause with file:line, planned fix, tests). No code edits until that entry
   exists and the user has agreed to start.
3. Branch and sync rules: `notes/08-branching-and-sync.md`. Commands and test
   setup: `notes/09-dev-setup.md`.

## Rules we keep from upstream's AGENTS.md

Because they protect our data and our ability to pull upstream fixes, not
because anything goes upstream:

- Deletion approval boundary. This fork runs against our real library. Never
  run the app, a scan or an import against a real library or instance
  without the user's explicit go-ahead for that exact target.
- Deterministic tests; focused test runs per crate (`09-dev-setup.md`).
- Both databases stay working (SQLite and Postgres), so upstream merges keep
  applying cleanly.
- **No schema migrations of our own while we still merge upstream.**
  Versions must be contiguous (`scryer-infrastructure-datastore/src/migrations/assets.rs:592`),
  so our 0342 and upstream's 0342 would collide on a live database. Repair
  data in code (scan, analysis, startup) instead. If a migration becomes
  unavoidable, stop and ask: it means we stop taking upstream migrations.
- Keep `skills/` in step with API changes we make.
- Keep changes at seams, small diffs in shared files, so upstream merges stay
  cheap.

Dropped: upstream PR rules, Markdown approval, issue-first comments,
outside-name bans, `fork:` commit prefixes.

## Commits

Signed (repo-local SSH signing is configured). `own/main` may be pushed
to the `fork` remote. Never commit a gitignored private path (`notes/`,
`parity/corpus/captured/`, `parity/capture/`, `parity/catalogue/*-{captured,config}.yaml`),
and never push `own/main-private-history`: it holds the pre-squash history
with that data, and `.githooks/pre-push` refuses it (D45). The pre-commit hook rejects
added lines containing the local username or home path, so write paths as
`~/...` in notes.
