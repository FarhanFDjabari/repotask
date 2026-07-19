# Contributing

Requires a stable Rust toolchain (1.82 or newer).

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt
```

Tests build real git repositories under a temp directory, so `git` must be on the PATH and
configured with a `user.name` and `user.email`.

## Design constraints

- **The CLI never calls a language model.** It locates, parses, budgets, and returns. Every command
  gathers evidence and states a contract; the agent supplies the judgement.
- **The JSON envelope is a contract.** `{schema, ok, command, data, warnings}` is what generated
  skills and agents depend on. Changing a field shape is a breaking change.
- **Knowledge lives in the knowledge base, not in the binary.** Skills reference commands; they
  never inline project conventions. The seed under `seed/` is a starting point for users to
  replace, not a set of rules RepoTask enforces.
- **Credentials never come from the project.** Environment first, then `~/.repo-task/secrets.yaml`.
- **Nothing is pushed on the user's behalf.** `kb propose` commits and prints the push command.

Keep provider APIs, automatic approvals, merge behaviour, deployment, and release decisions out of
RepoTask. Add project-specific behaviour through knowledge-base slices and fact families rather
than generic assumptions in the tool.
