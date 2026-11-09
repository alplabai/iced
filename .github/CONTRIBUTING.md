# Contributing to the Alp Lab fork

This repository is Alp Lab's fork of [iced](https://github.com/iced-rs/iced), used by [Signex](https://github.com/alplabai/signex). These rules apply to every change in this fork. Changes offered to `iced-rs/iced` itself also follow [upstream's contributing guide](https://github.com/iced-rs/iced/blob/master/CONTRIBUTING.md); see Upstreaming below.

## Starting a change

All discussion happens on GitHub in this repository. There is no separate chat.
- **Bugs and concrete changes:** open an issue. Say what is wrong or missing, why Signex (or another user) needs it, and the area of the code.
- **Ideas, design questions and larger changes:** open a [Discussion](https://github.com/alplabai/iced/discussions) first. Once there is agreement, turn it into an issue.
- Work starts from an issue. Comment that you are taking it (or get it assigned) before you open a branch, so two people don't do the same work.

## Branches

| Branch | Role | How it changes |
|---|---|---|
| `master` | Mirror of upstream `iced-rs/iced` `master`. Never edited by hand. | Fast-forwarded from upstream only. |
| `dev` | Default branch and integration branch. All work starts here. | Pull requests only. |
| `signex` | Release branch. Signex pins a commit or tag from this branch. | Pull requests from `dev` only. |

- **Work:** branch off `dev` (`fix/<issue>-<topic>`, `feat/<issue>-<topic>`, `upstream/<pr>-<topic>`), open the pull request against `dev`, squash-merge.
- **Release:** when `dev` is ready for Signex, open a pull request `dev` → `signex` and merge it with a merge commit (no squash, so the history stays shared). Then tag the merge commit `iced-signex-v<version>` (for example `iced-signex-v0.15.0-alpha.3`) and pin that tag's commit in Signex's `[patch.crates-io]`.
- **Upstream:**
  - From time to time, fast-forward `master` from `iced-rs/iced`, then open a pull request `master` → `dev` to bring the new upstream work in.
  - An upstream pull request that Signex needs before upstream merges it is ported on its own `upstream/<pr>-<topic>` branch into `dev`, and recorded in `FORK.md` under "Merged upstream PRs".
- **Hotfix:** branch off `signex`, open a pull request into `signex`, then bring the same fix into `dev` with a second pull request so the two branches do not diverge.

CI: `.github/workflows/fork-ci.yml` runs fmt, clippy (`cargo lint`), `cargo nextest` with all features plus doc tests, and a wasm check, on Ubuntu, for pull requests into and pushes to `dev`/`signex`. Its summary check `fork-ci · required` must pass before merging. The upstream workflows are disabled in the Actions settings, so they don't use the org's shared runner budget; their files stay untouched to keep upstream syncs conflict-free.

Formatting: upstream `master` is always `cargo fmt` and `cargo lint` clean, and the fork uses upstream's `rustfmt.toml` and lint alias unchanged. Never reformat or re-lint code the fork did not add. Format only your own changes, so upstream syncs stay conflict-free. Fork CI checks the whole tree, so a red fmt or clippy result always points at fork-added code.

Protection (repository rulesets):
- `dev` and `signex` need a pull request with one approval and resolved review threads, and cannot be force-pushed or deleted.
- `master` cannot be deleted.
- `iced-signex-*` tags cannot be moved or deleted.
- Organization admins can bypass these rules for emergencies only.

## Upstreaming

When we fix or build something here first, we offer it upstream, so the fork stays small and upstream gains the fix.
- Write fork changes in upstream style: one concern per commit, no Signex-specific code, tests included, and no dependency on other fork-only changes where possible.
- After a change is merged into `dev` and proven in Signex, rebase it onto upstream `master` on an `upstream-offer/<topic>` branch, and open the pull request against `iced-rs/iced`. Follow [upstream's contributing guide](https://github.com/iced-rs/iced/blob/master/CONTRIBUTING.md), including discussing the idea with the upstream core team first.
- Every upstream pull request is posted publicly, so it needs a maintainer's OK (Hakan or Caner) before it is opened.
- Track the state in `TRIAGE.md` under "Upstream watch". When upstream merges an equivalent change, drop ours at the next `master` → `dev` sync.

## Pull requests

- Every fork change has an issue. Title the pull request `Issue #N: <what changes> (fixes #N)`.
- Add a line to `FORK.md` under "Fork fixes" (or "Merged upstream PRs" / "Skipped") in the same pull request.
- Add a changelog fragment, `changelog.d/<PR number>.<section>.md`, with one bullet that ends in a `Signex:` sentence saying what changes for Signex. The release notes are assembled from these fragments; see [`changelog.d/README.md`](../changelog.d/README.md). Pull requests that only touch docs or CI need none.
- Keep a fork fix small and generic: code that belongs in iced lives here, and Signex-specific policy stays in Signex.
- Before asking for review, run `cargo check --workspace --all-targets` and the tests of every crate you touched with the features they need (for example `cargo test -p iced_graphics --features geometry`).
- Say whether the change was checked on a real GPU or only on software rendering.
- Commits and pull requests carry no AI attribution (no `Co-Authored-By` trailers for tools, no generated-by footers).
