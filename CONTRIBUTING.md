# Contributing

Thanks for contributing to Blocky.

## Development loop

1. Format code:
   ```bash
   cargo fmt
   ```
2. Lint code:
   ```bash
   cargo clippy --workspace --all-targets -- -D warnings
   ```
3. Run tests:
   ```bash
   cargo test --workspace
   ```
4. Verify the SDK builds for Wasm:
   ```bash
   cargo build -p blocky-sdk --target wasm32-unknown-unknown
   ```

## Notes

- Follow conventional commits: `type(scope): description`
- Keep changes focused and commit in logical parts
- Prefer `tracing` over `println!`
- Run the checks above before opening a pull request
- For major architecture or semantics changes, write or update an RFC in `docs/`; see `docs/rfc-process.md`
