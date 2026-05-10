---
name: commit
description: "Commit staged changes, push the branch, and create or update a PR with a conventional-commit-formatted title. Use after completing a feature or fix."
---

# Commit, Push & PR

This skill handles the full workflow: verify quality gates → commit → push → create or update a PR.

## Step 1: Quality Gates

Run all checks before committing. All must pass:

```sh
make build     # must compile cleanly
make test      # all tests must pass
make lint      # zero warnings
make fmt-check # code formatted
```

Stop if any check fails. Fix the issue, then re-run.

## Step 2: Create a Feature Branch

**Always work on a feature branch — never commit directly to `main`.**

Check the current branch:

```sh
git branch --show-current
```

If already on `main` (or any protected branch), create and switch to a feature branch before staging anything. Derive the branch name from the commit type and a short summary of the change (kebab-case, no special characters):

```sh
git checkout -b type/short-description
# e.g.: feat/auth-flow, fix/token-output, refactor/database-layer
```

If already on a feature branch, continue with that branch — do not create another one.

## Step 3: Review Changes

```sh
git status && git diff --staged && git diff
```

Understand what changed so you can write an accurate commit message and PR title.

## Step 4: Stage & Commit

Stage relevant files (prefer specific paths over `git add -A` to avoid accidentally including secrets or build artifacts):

```sh
git add <files...>
```

Write a conventional commit message:

```
type(scope): summary in imperative mood
```

**Changelog-eligible types** (pick the right one — it determines what appears in the changelog):

| Type | Changelog section | Version bump |
|------|-------------------|--------------|
| `feat` | Added | minor |
| `fix` | Fixed | patch |
| `perf` | Performance | patch |
| `docs` | Documentation | none |
| `test` | Tests | none |
| `refactor`, `chore`, `ci`, `build`, `style` | *(not included)* | none |

For breaking changes use `feat!:` or `fix!:`, or add a `BREAKING CHANGE:` footer → triggers a major version bump.

Scopes are lowercase, comma-separated if multiple: `feat(api,auth): ...`

```sh
git commit -m "type(scope): summary"
```

## Step 5: Push

```sh
git push -u origin HEAD
```

## Step 6: Create or Update the PR

**Check if a PR already exists for this branch:**

```sh
gh pr view --json number,title,url 2>/dev/null
```

### If no PR exists — create one:

The PR title **must** follow conventional commit format — it becomes the squashed commit message on `main` and is what drives the changelog. Match it to the overall intent of the branch, not just the latest commit.

```sh
gh pr create \
  --title "type(scope): summary" \
  --body "$(cat <<'EOF'
## Summary

<brief description of the changes and motivation>

## Test plan

- [ ] `make build` passes
- [ ] `make test` passes
- [ ] `make lint` has zero warnings
- [ ] `make fmt-check` applied

## Checklist

- [ ] Tests added/updated
- [ ] Documentation updated (if user-facing behavior changed)
- [ ] Commit messages follow conventional commit style
EOF
)"
```

### If a PR already exists — update it:

Re-evaluate the PR title and description to reflect the **combined** scope of all commits on the branch, then update:

```sh
gh pr edit \
  --title "type(scope): updated summary" \
  --body "$(cat <<'EOF'
## Summary

<updated description covering all changes>

## Test plan

- [ ] `make build` passes
- [ ] `make test` passes
- [ ] `make lint` has zero warnings
- [ ] `make fmt-check` applied

## Checklist

- [ ] Tests added/updated
- [ ] Documentation updated (if user-facing behavior changed)
- [ ] Commit messages follow conventional commit style
EOF
)"
```

## Key Reminders

- **PR title = squashed commit on main = changelog entry.** Choose the type and summary carefully.
- The individual commits within the branch don't appear in the changelog — only the PR title does.
- If the branch touches multiple scopes, use comma-separated scopes: `feat(api,auth): ...`
- Never skip hooks (`--no-verify`) — fix the underlying issue instead.

## Tracking mechanism

This skill is a workflow helper, not a drift sync. `.agent/skills/commit/.last-updated` is therefore intentionally empty: there is no baseline to diff against because the skill has no source of truth to mirror. The §21.3 file exists only to satisfy the structural check; the trigger for this skill is "the user finished a unit of work and wants it committed and pushed", not "the baseline drifted."

## Discovery process

Invoke the skill when **all** of the following are true:

```sh
git diff --quiet HEAD || echo "unstaged or staged changes present"
git rev-parse --abbrev-ref HEAD                # not on main / master / a protected branch
git log --oneline @{u}..HEAD 2>/dev/null       # local commits not yet pushed (or no upstream yet)
```

If the working tree is clean, the branch is `main`, or the branch already has an open PR matching the current scope, the skill has nothing to do.

## Mapping table

Maps the **change shape** to the conventional-commit `type` to use in the commit message and PR title (this drives the changelog entry — see Step 4):

| Change shape                                                                | `type`     | Changelog section |
|-----------------------------------------------------------------------------|------------|-------------------|
| New user-visible capability (command, flag, subcommand, output mode)        | `feat`     | Added             |
| Bug fix in shipped behaviour                                                 | `fix`      | Fixed             |
| Performance improvement with no behaviour change                            | `perf`     | Performance       |
| Documentation-only change (`README.md`, `docs/`, `man/`, `prompts/`, etc.)  | `docs`     | Documentation     |
| Test-only change                                                             | `test`     | Tests             |
| Refactor with no observable behaviour change                                | `refactor` | *(omitted)*       |
| Build/CI/tooling/configuration                                               | `chore` / `ci` / `build` | *(omitted)* |

Breaking change in any row → append `!` (`feat!:`, `fix!:`) or include a `BREAKING CHANGE:` footer; this triggers a major version bump.

## Verification

After running the skill:

1. `git status` reports a clean working tree.
2. `git log @{u}..HEAD` is empty (the branch is fully pushed).
3. `gh pr view --json number,url,title` returns a PR whose `title` matches the conventional-commit format and whose `state` is `OPEN`.
4. The PR's most recent CI run is `queued`, `in_progress`, or `success` — never `failure` without an investigation comment from the agent.

## Skill self-improvement

After every run, update this file when:

1. **Conventional-commit conventions change** in the project (e.g. a new `type` is added, scopes are restructured) — update the mapping table above and Step 4's table together.
2. **The PR template** (`.github/PULL_REQUEST_TEMPLATE.md`) is rewritten — refresh the heredoc bodies in Step 6 so the generated PR matches.
3. **The quality-gate command set** (`make build`/`test`/`lint`/`fmt-check`) changes — refresh Step 1 and the Test plan blocks in Step 6.
4. **Branch-protection rules or merge strategy** change — update Step 2 (branch naming) and the "Key Reminders" note about squash merges.

Commit any skill edits in the same PR as the change that prompted them.
