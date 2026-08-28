# Agent instructions (this repo)

This file overrides generic skill defaults for **this repository**.

## Plans and review

- Execute numbered plans in `docs/superpowers/plans/` **in order**.
- Inside one plan file: implement **every task**, then **one review** of that plan's full commit range.
- **Do not** dispatch a reviewer after each task.
- Do not start the next plan until the current plan's tests pass and that plan-level review is done.

The Superpowers subagent-driven-development skill still applies for implementers, worktrees, and ledgers. The per-task review loop in that skill is **replaced** by the rule above.
