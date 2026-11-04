# Contributing to the Alp Lab fork

This repository is Alp Lab's fork of [iced](https://github.com/iced-rs/iced), used by [Signex](https://github.com/alplabai/signex). The rules below apply to every change in this fork. The upstream iced guide follows them and applies only to contributions sent upstream.

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
- After a change is merged into `dev` and proven in Signex, rebase it onto upstream `master` on an `upstream-offer/<topic>` branch, and open the pull request against `iced-rs/iced`. Follow upstream's CONTRIBUTING (below), including discussing the idea with the core team first.
- Every upstream pull request is posted publicly, so it needs a maintainer's OK (Hakan or Caner) before it is opened.
- Track the state in `TRIAGE.md` under "Upstream watch". When upstream merges an equivalent change, drop ours at the next `master` → `dev` sync.

## Pull requests

- Every fork change has an issue. Title the pull request `Issue #N: <what changes> (fixes #N)`.
- Add a line to `FORK.md` under "Fork fixes" (or "Merged upstream PRs" / "Skipped") in the same pull request.
- Keep a fork fix small and generic: code that belongs in iced lives here, and Signex-specific policy stays in Signex.
- Before asking for review, run `cargo check --workspace --all-targets` and the tests of every crate you touched with the features they need (for example `cargo test -p iced_graphics --features geometry`).
- Say whether the change was checked on a real GPU or only on software rendering.
- Commits and pull requests carry no AI attribution (no `Co-Authored-By` trailers for tools, no generated-by footers).

---

# Contributing

Thank you for considering contributing to Iced! Take a look at [the roadmap] to get an idea of the current state of the library.

The core team is busy and does not have time to mentor nor babysit new contributors. If a member of the core team thinks that reviewing and understanding your work will take more time and effort than writing it from scratch by themselves, your contribution will be dismissed. It is your responsibility to communicate and figure out how to reduce the likelihood of this!

The general advice for new contributors is to share your ideas with the community. You can share your ideas and gather feedback in [our Zulip forum]. This is a very important step. It helps to coordinate work, get on the same page, and start building trust. Remember that [Code is the Easy Part] and also [The Hard Parts of Open Source]!

Once you have started a channel of communication, you must wait until someone from the core team chimes in. If the core team is busy, this can take a long time (maybe months!). Your idea may need a bunch of iteration, or it may turn into something completely different, or it may be completely discarded! You will have to be patient and humble. Remember that open-source is a gift.

Besides directly writing code, there are many other different ways you can contribute. To name a few:

- Writing tutorials or blog posts
- Improving the documentation
- Submitting bug reports and use cases
- Sharing, discussing, researching and exploring new ideas or crates

[the roadmap]: ROADMAP.md
[our Zulip forum]: https://iced.zulipchat.com/
[Code is the Easy Part]: https://youtu.be/DSjbTC-hvqQ?t=1138
[The Hard Parts of Open Source]: https://www.youtube.com/watch?v=o_4EX4dPppA
