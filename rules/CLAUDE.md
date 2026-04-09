# Rules for Claude Sessions on Rustify

## Session Context

When starting a new session on this project, read:
1. `docs/TUI.md` — what exists, architecture, what's next
2. `rules/DEVELOPMENT.md` — code patterns, file organization, testing approach
3. The relevant spec in `docs/superpowers/specs/` if implementing a specific tier

## How This User Works

- Prefers bottom-up development: types first, then modules, then integration
- Writes design specs before implementation
- Wants concise answers — don't over-explain
- Approves designs with "yes" or "yeah" — take that as full approval and proceed
- When told "go ahead" — proceed with implementation, don't ask more questions
- Prefers Fisher-Yates shuffle, three-mode repeat cycle (Off/All/One), and standard UX patterns

## Implementation Approach

- Implement directly in the main repo at `C:\Users\att1a\WS\rustify` — do NOT use git worktrees for implementation (agents that spawn subprocesses may default to the worktree path instead of the main repo)
- When using subagents, always specify `Work from: C:\Users\att1a\WS\rustify` explicitly
- Build and test with `cargo test --workspace` after each change
- Commit frequently with conventional commit messages
- All core changes need tests. TUI snapshot tests for visual changes.

## What NOT to Do

- Don't add features beyond what was asked
- Don't refactor code you're not changing
- Don't add comments, docstrings, or type annotations to untouched code
- Don't create README.md files unless asked
- Don't use async/tokio — the project is deliberately sync with crossbeam channels
- Don't add MPRIS implementation on Windows — it's a Linux-only feature
