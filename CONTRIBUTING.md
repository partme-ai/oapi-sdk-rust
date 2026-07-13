# Contributing

1. Create a focused branch from `main`.
2. Run formatting, Clippy, tests and documentation checks.
3. Add tests for protocol behavior and public builders.
4. Keep secrets and real application credentials out of fixtures.
5. Open a pull request describing behavioral compatibility with the official SDKs.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
```

Generated service modules should not implement their own authentication or HTTP stack; they must build `ApiRequest` and delegate to `Client`.
