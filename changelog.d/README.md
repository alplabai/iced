# Changelog fragments

Every pull request into `dev` that changes behaviour adds a fragment here. At the next `dev` → `signex` release, the fragments are folded into that release's `RELEASE-iced-signex-v<version>.md` and deleted. Each release's notes therefore say what changed for Signex without anyone reconstructing it afterwards. `FORK.md` stays the permanent per-change record. `CHANGELOG.md` is upstream's file, so do not edit it.

This mirrors Signex's own `changelog.d/`.

## Naming

```
changelog.d/<number>.<section>.md        # 84.fixed.md
changelog.d/<number>.<section>.<k>.md    # 84.fixed.2.md  (second fragment, same number and section)
```

- `<number>` is the pull request number. Use the issue number only if you need the file before the pull request exists.
- `<section>` is one of the following:
  - `upstream`: a ported or merged upstream iced pull request;
  - `added`, `changed`, `fixed`, `removed`: Keep a Changelog types, for fork changes.
- A pull request may add several fragments, one per section.

## Content

Only the bullet, with no heading:

```
- **Bold lead (#84)** — one or two sentences on what changed. Signex: what this fixes or changes for Signex, or "no Signex code path uses this".
```

- The `Signex:` sentence is required. It says what changed for Signex: which screen or code path benefits, or that none does.
- A breaking change (a changed public API, a removed item, a changed default) starts its bullet with **Breaking:**. A behaviour change that is not breaking says **Behaviour change:** in the bullet. The release notes lift both into their own sections.

## Checking and collecting

```
bash changelog.d/assemble.sh --check   # validate names and bullets (Fork CI runs this)
bash changelog.d/assemble.sh           # print all fragments grouped by section
```

When writing the release notes, paste the printed sections into `RELEASE-iced-signex-v<version>.md`. Delete the fragments in the same pull request: `git rm changelog.d/[0-9]*.md`.
