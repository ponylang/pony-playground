---
name: dependabot-updates
description: Triage and merge open Dependabot PRs for this repository. Verifies each PR is genuinely from Dependabot, closes spoofed or suspicious ones, and merges the clean ones one at a time — requesting a rebase before each subsequent merge so CI runs against a main that already includes this session's earlier dependency merges. Invoke explicitly with /dependabot-updates.
disable-model-invocation: true
---

# Dependabot Updates

Handle the repository's open Dependabot pull requests end to end: verify each is
genuinely from Dependabot, close spoofed or suspicious ones, hand back any that a human
has pushed commits to, and merge the rest one at a time. This skill runs autonomously —
it closes and merges PRs without stopping to ask — so it only runs when a human invokes
`/dependabot-updates`, never on the model's own initiative.

Run it from within the repository checkout; the repository is taken from the `origin`
remote, not hardcoded. Capture it once:

```bash
REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
```

All GitHub state is read with `gh`. Re-running is safe: merged and closed PRs drop out of
`gh pr list --state open`, so a second run picks up only what is left.

## Scope: what this does and does not check

The identity and diff checks below catch a **spoofed or tampered pull request** — a PR
that pretends to be Dependabot, or a genuine Dependabot PR whose diff has been altered to
touch things a dependency bump never should. They do **not** vet the upstream release
itself. A published dependency version can be malicious even when the PR that bumps to it
is a perfectly ordinary, genuine Dependabot diff. Auditing upstream releases is a
supply-chain concern outside a PR-diff review and is out of scope for this skill.

## Step 0 — Find candidate PRs

```bash
gh pr list --state open --json number,author,headRefName,isDraft,title
```

A **candidate** is any non-draft PR that presents as Dependabot:

- `headRefName` starts with `dependabot/`, or
- `author.login` is `app/dependabot` (this is how `gh pr list` renders the account; the
  REST API in Step 1 spells the same account `dependabot[bot]`), or
- `title` matches `^Bump .+ from .+ to .+`.

Ignore everything else — ordinary human PRs are not this skill's concern. If there are no
candidates, report "no open Dependabot PRs" and stop.

Casting this wide net matters: a spoof that mimics Dependabot must land in the candidate
set so Step 1 can catch it, rather than being filtered out as "not Dependabot."

## Step 1 — Triage each candidate

Run all three checks below on every candidate before moving to Step 2. In every command
below, `$N` is the number of the candidate PR currently being processed.

### 1a. Identity — the anti-spoof gate

```bash
gh api repos/$REPO/pulls/$N --jq \
  '{login: .user.login, type: .user.type, head_ref: .head.ref, head_repo: .head.repo.full_name, base_repo: .base.repo.full_name}'
```

The PR is genuinely from Dependabot only if **all** hold:

- `login == "dependabot[bot]"` and `type == "Bot"`, and
- `head_ref` starts with `dependabot/`, and
- `head_repo == base_repo` (the branch lives in this repository, not a fork).

This is the load-bearing check. A human cannot open a pull request *as* the
`dependabot[bot]` app account, and the app slug `dependabot` is reserved by GitHub, so
these fields cannot be forged. The in-repo-branch requirement rejects a fork branch named
`dependabot/...` that mimics the real thing.

Do **not** use commit GPG signatures as the identity anchor. `verification.verified ==
true` only proves that *some* account-registered key signed the commit — a contributor's
own key qualifies — so it neither proves Dependabot nor, when absent, proves a spoof.

**If identity fails**, this is a spoof presenting as Dependabot. Close it:

```bash
gh pr close $N --repo $REPO --comment \
  "This PR presents as a Dependabot update but fails identity verification (author is not the dependabot[bot] app account, or the branch is from a fork). Closing as a likely spoof. If this is a mistake, reopen and handle it manually."
```

Record it and do not process it further.

### 1b. Commit provenance

```bash
gh api repos/$REPO/pulls/$N/commits --jq '[.[].author.login] | unique'
```

Every commit's `author.login` should be `dependabot[bot]`. The gate is the **author**
login, not the committer: a genuine Dependabot commit has `committer.login == "web-flow"`
and `committer.name == "GitHub"`, so checking the committer login against `"GitHub"` would
reject every real Dependabot PR.

If a commit has a different author, someone pushed to the Dependabot branch by hand. That
PR is no longer a pure dependency bump. **Skip it and flag it for the human** — do not
merge it, and do not close it either, because this is most likely a maintainer's
legitimate fixup and closing would throw their work away.

### 1c. Suspicious diff

Read the ecosystem from the branch directory in `headRefName` — `dependabot/<dir>/...`,
where `<dir>` is `cargo`, `github_actions`, `npm_and_yarn`, `docker`, `pip`, `bundler`,
and so on — and the changed files:

```bash
gh pr view $N --repo $REPO --json files --jq '.files[].path'
```

A dependency bump only ever touches that ecosystem's manifest and lock files. Match by
basename or path so nested layouts (for example a Cargo workspace's `crates/*/Cargo.toml`)
still match:

- **cargo** (`cargo`): `Cargo.toml`, `Cargo.lock`
- **github-actions** (`github_actions`): files under `.github/workflows/` or `.github/actions/`
- **npm** (`npm_and_yarn`): `package.json`, `package-lock.json`, `yarn.lock`, `pnpm-lock.yaml`
- **docker** (`docker`): `Dockerfile*`, `docker-compose*`
- **pip** (`pip`): `requirements*.txt`, `pyproject.toml`, `Pipfile*`, `poetry.lock`, `setup.py`, `setup.cfg`
- **bundler** (`bundler`): `Gemfile`, `Gemfile.lock`

**Close the PR as suspicious** when any of these hold:

- a changed file falls outside the ecosystem's set above, or
- the bump is a downgrade (compare the "from" and "to" versions in the title), or
- the target is a pre-release or a version known to be yanked.

```bash
gh pr close $N --repo $REPO --comment \
  "Closing as suspicious: this verified Dependabot PR <state the specific anomaly, e.g. 'changes files outside Cargo.toml/Cargo.lock' or 'is a version downgrade'>, which a routine dependency bump should not do. If this is expected, reopen and merge manually."
```

State the specific anomaly in the comment — never a bare "suspicious." Record it and do
not process it further.

An unusually large diff for a single-dependency bump is worth noting in the final report,
but do not close on diff size alone — lock-file churn varies and size is a weak signal.

## Step 2 — Merge the survivors, one at a time

Process survivors in ascending PR-number order. Keep a `mergedAny` flag, starting false;
it becomes true only after a successful merge in this run. It — not "is this the first
PR" — decides whether a rebase is needed, because the reason to rebase is that a merge
moved `main`. If an earlier survivor was skipped instead of merged, `main` has not moved
and the next one needs no rebase.

### 2a. Rebase (only when `mergedAny` is true)

Record the current head, ask Dependabot to rebase, and wait for it:

```bash
OLD_SHA=$(gh pr view $N --repo $REPO --json headRefOid -q .headRefOid)
gh pr comment $N --repo $REPO --body "@dependabot rebase"
```

Then poll (roughly every 30s, capped at ~15 minutes). Each poll, re-read the head SHA and
Dependabot's latest reply comment:

```bash
gh pr view $N --repo $REPO --json headRefOid -q .headRefOid
gh pr view $N --repo $REPO --json comments \
  --jq '[.comments[] | select(.author.login | ascii_downcase | startswith("dependabot"))] | last | .body'
```

Stop on the first of these:

- the head SHA differs from `OLD_SHA` → rebase done; note the time of the change for the
  CI floor in 2b.
- Dependabot's reply says the PR is already up to date → no new commit; proceed.
- Dependabot's reply says it cannot rebase because of a conflict → record a **rebase
  conflict** (distinct from a timeout), skip this PR, continue.
- the cap is reached → record a **rebase timeout**, skip this PR, continue.

After a rebase, re-run 1b (provenance) and 1c (suspicious diff) on the new head — a push
during the wait could have changed either.

### 2b. Verify CI

Read the checks as structured data and never rely on the exit code alone:

```bash
gh pr checks $N --repo $REPO --json name,state,bucket
```

`bucket` sorts each check into `pass`, `fail`, `pending`, `skipping`, or `cancel`
(`skipping` covers a neutral result such as a skipped CodeQL run).

Handle these cases in order:

- **No checks yet.** When a PR has no checks, `gh pr checks` prints
  `no checks reported on the '<branch>' branch` to stderr with empty stdout and exit 1 —
  it does not print `[]`. Treat empty or non-JSON stdout as "checks have not registered
  yet, keep waiting," never as a pass and never as a failure. (Exit 1 also means real
  failure and exit 8 means pending, so the exit code by itself is ambiguous; the JSON
  content is authoritative.)
- **Not yet settled.** After a rebase, the new SHA's check runs register incrementally,
  and not every workflow re-runs on a rebase (a `pull_request_target` label workflow, for
  example, does not fire on `synchronize`, while the `pull_request` build and lint
  workflows do). So a fast check can be `pass` while the slow build check has not been
  created yet. Do **not** merge on the first all-terminal read. Require all of:
  - the check set is non-empty and every bucket is terminal (`pass`, `fail`, `skipping`,
    or `cancel`), and
  - the set of check names is identical across two consecutive polls ~30s apart, and
  - the CI floor is met: if this PR was rebased in 2a, at least ~60s have passed since
    that head-SHA change (measure from the SHA change, not from when polling started). If
    no rebase happened this run (the first survivor), there is no new push to race — the
    checks are already registered from the PR's last push — so the floor does not apply;
    the non-empty, all-terminal, and stable-name-set requirements still do.

  Do not anchor on a hardcoded set of expected check names: the set differs between the
  PR's original open and a rebase, so a pre-rebase snapshot would wait forever. This
  narrows but does not entirely eliminate the window where a check registers unusually
  late; note that residual honestly if it ever bites.
- **Still pending or not settled** → keep waiting, capped at ~15–20 minutes. On expiry,
  record "CI did not complete," skip this PR, continue.
- **Settled with any `fail` or `cancel`** → record which check failed and its run URL,
  skip this PR, continue. (`mergedAny` is unchanged — nothing merged.) A housekeeping
  check counts here too; skipping on any red check is the safe choice — it never merges
  red and it surfaces the PR to the human.
- **Settled with only `pass` and `skipping`** → the PR passed CI; go to 2c.

### 2c. Merge

Confirm the PR is actually mergeable, then squash-merge (squash is the only merge strategy
the ponylang org allows):

```bash
gh pr view $N --repo $REPO --json mergeable,mergeStateStatus
gh pr merge $N --repo $REPO --squash --delete-branch
```

If it is not mergeable (for example `mergeStateStatus` is `DIRTY` from a conflict
unrelated to CI), skip it and record why. If `gh pr merge` errors, catch the error and
record it — do not abort the whole run. On success, set `mergedAny = true` and move to the
next survivor.

## Step 3 — Report

This skill runs autonomously, so its final report is how the human learns what happened.
Make it complete and specific:

- candidates found;
- spoofs closed, with the reason each failed identity;
- suspicious PRs closed, with the specific anomaly each had;
- PRs skipped and flagged (non-Dependabot commits), so the human can handle them;
- PRs merged;
- PRs skipped on CI failure, with the failing check and its run URL;
- rebase conflicts and rebase timeouts, kept distinct;
- non-mergeable skips and any merge errors;
- anything else that needs a human.
