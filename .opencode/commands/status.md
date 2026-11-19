---
description: Report current project state — git status, WIP, todos, blockers. Read-only.
---

Report status without changing anything.

1. `git status --short`, `git log --oneline -5`, `git diff --stat`, and `git diff --stat --staged`.
2. Current todo list state, if any.
3. Summarize: what changed since the last commit, what is in progress, what is blocked.
4. If the tree is fmt/clippy/test-dirty, say so and list the failing command(s) with exit codes. Do not fix anything in this command.
