# Contributing

Thanks for wanting to contribute.
One rule up front:

**Human-authored pull requests targeting `main` must be raised through [`no-mistakes`](https://github.com/kunchenguid/no-mistakes).**

`no-mistakes` puts a local Git proxy in front of the real remote. Pushing through it runs an AI-driven review, test, and build pipeline in an isolated worktree, forwards the push only after every check passes, and opens a clean pull request automatically. Fork-based contributions require no-mistakes **v1.30.1** or newer.

The `Require no-mistakes` GitHub Actions check verifies the deterministic signature that no-mistakes writes into the pull-request body. Known release and dependency bots are exempt.

## Workflow

1. Fork the repository, then clone the parent repository or set local `origin` to `git@github.com:aivv73/jj-axi.git`.
2. Create a branch and make your changes.
3. Initialize or refresh the gate with your fork as the push target:

   ```sh
   no-mistakes init --fork-url git@github.com:<you>/jj-axi.git
   ```

4. Commit your changes.
5. Push through the gate instead of pushing to `origin`:

   ```sh
   git push no-mistakes
   ```

6. Run `no-mistakes` to attach to the pipeline and address its findings.
7. Once the pipeline passes, it pushes the branch to your fork and opens the pull request against this repository.

See the [no-mistakes quick start](https://kunchenguid.github.io/no-mistakes/start-here/quick-start/) for the full first-run walkthrough.

## Development requirements

- Rust 1.89 or newer;
- Jujutsu 0.43.0 available as `jj` on `PATH`;
- Git;
- `gh` for GitHub integration work.

Run the same checks as CI before pushing:

```sh
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
cargo package --locked
git diff --check
```

## Repository conventions

- Keep `Cargo.lock` committed and update it intentionally.
- Add integration coverage under `tests/` for CLI behavior.
- Preserve structured, non-interactive output and error contracts.
- Keep the canonical agent skill in `skills/jj-axi/SKILL.md` synchronized with user-facing command behavior.
- Document significant architectural decisions under `docs/adr/` and update `CONTEXT.md` when domain terminology changes.

## Questions and bug reports

Open a [GitHub issue](https://github.com/aivv73/jj-axi/issues).
