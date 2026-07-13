# Build and validation report

Validation date: `2026-07-13`

This document records what was actually executed for `oapi-sdk-rust` 0.1.0, what each check covers, and what remains a manual end-to-end responsibility.

## 1. Tested toolchain

```text
rustc 1.85.0 (4d91de4e4 2025-02-17)
cargo 1.85.0 (d73d2caf9 2024-12-31)
rustfmt 1.8.0-stable
clippy 0.1.85
```

The repository pins the toolchain channel in `rust-toolchain.toml` and declares `rust-version = "1.85"` in `Cargo.toml`.

## 2. Validation summary

| Check | Result | Scope |
|---|---:|---|
| Workspace metadata parse | Passed | Root crate and example workspace |
| `cargo fmt --check` | Passed | All Rust source files |
| All-feature type check | Passed | Transport, registration, events, example |
| Example binary build | Passed | `create-an-app-in-one-click-rust` |
| Unit tests | Passed, 5/5 | Registration and event utilities |
| Strict Clippy | Passed, zero warnings | Workspace, all targets, all features |
| Rustdoc with warnings denied | Passed | Public API documentation |
| Feature-isolation checks | Passed | Minimal, registration-only, events-only combinations |
| crates.io package creation | Passed | `lark-oapi-0.1.0.crate` |
| Live QR registration | Not run | Requires real user consent and creates/modifies a real app |

## 3. Commands executed

All commands used a generated `Cargo.lock`:

```bash
cargo generate-lockfile
cargo fmt --all -- --check
cargo check --workspace --all-features --locked
cargo build -p create-an-app-in-one-click-rust --locked
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
```

Feature-isolation checks:

```bash
cargo check -p lark-oapi --no-default-features --locked
cargo check -p lark-oapi --no-default-features --features rustls-tls --locked
cargo check -p lark-oapi --no-default-features --features rustls-tls,registration --locked
cargo check -p lark-oapi --no-default-features --features rustls-tls,events --locked
```

Package validation:

```bash
cargo package -p lark-oapi --allow-dirty --no-verify
```

The package artifact was generated as:

```text
target/package/lark-oapi-0.1.0.crate
```

`--allow-dirty` was necessary in the isolated build environment because the source tree was not initially attached to the final GitHub repository. The package file list and metadata were still produced from `Cargo.toml` include rules.

## 4. Unit-test coverage

Five library tests passed. They cover:

### Registration

- QR URL construction keeps existing query parameters;
- `from=sdk`, `tp=sdk`, and `source=rust-sdk/...` parameters;
- application name/description/avatar URL encoding;
- Addons JSON normalization, gzip compression, URL-safe Base64 without padding;
- invalid/empty Addons rejection;
- avatar and input validation behavior.

### Events

- callback signature concatenation order;
- SHA-256 signature value;
- request Header signature verification.

The tests use local deterministic data and do not require Feishu/Lark credentials.

## 5. Compile-time coverage

The all-feature build validates:

- reqwest + rustls transport;
- app/tenant/user token request paths;
- pluggable async `TokenCache`;
- JSON, form, bytes, multipart request construction;
- generic API response decoding and structured errors;
- typed IM message service;
- registration session and polling API;
- event signature/decryption parser;
- terminal QR example dependencies and environment configuration.

The feature-isolation checks ensure that optional registration/event cryptographic dependencies are not required when the corresponding features are disabled.

## 6. Static quality gates

### Formatting

```bash
cargo fmt --all -- --check
```

Result: no formatting differences.

### Clippy

```bash
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

Result: zero warnings. `-D warnings` converts every Clippy/rustc warning into a failure.

### Documentation

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
```

Result: public documentation generated without warnings.

## 7. GitHub Actions

`.github/workflows/ci.yml` runs repository checks on GitHub-hosted Linux, macOS, and Windows environments. Each job generates a lockfile before enforcing `--locked` checks.

Local validation is the recorded source of truth for the initial import. The GitHub Actions result must also be checked after the first full-source commit.

## 8. Security checks performed

- `Config::Debug` redacts `app_secret`;
- cached token `Debug` output redacts token values;
- `RegisterAppResult::Debug` redacts `client_secret`;
- request tracing records method/path but not Authorization Header values;
- Access Token Header values are marked sensitive in `http::HeaderValue`;
- `.env` is excluded by `.gitignore`;
- source and documentation contain placeholders rather than real credentials.

This is not a full third-party security audit. Dependency advisories and supply-chain checks should be added to CI, for example with `cargo audit` or `cargo deny`.

## 9. Intentionally excluded live tests

A real one-click registration test was not run automatically because it:

- requires a human to scan a QR code and approve the operation;
- creates or modifies a real Feishu/Lark application;
- returns a real App Secret that must be handled as sensitive data;
- depends on tenant policy and the scanning user's permissions.

A live acceptance run should follow the checklist in [create-an-app-in-one-click-rust.md](create-an-app-in-one-click-rust.md#13-实际验收清单).

## 10. Known validation limits

The initial report does not claim:

- every Feishu/Lark OpenAPI has a generated Rust type;
- marketplace flows have been exercised against a live App Ticket and tenant;
- WebSocket long connection support exists;
- client assertion OAuth exists;
- the library has already been published to crates.io;
- the library has undergone an external penetration or cryptographic audit.

The generic `ApiRequest` layer provides access to APIs without typed wrappers, but endpoint-specific runtime behavior should be covered as new services are generated.

## 11. Reproduce locally

```bash
git clone https://github.com/partme-ai/oapi-sdk-rust.git
cd oapi-sdk-rust
rustup show
cargo generate-lockfile
cargo fmt --all -- --check
cargo check --workspace --all-features --locked
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
```

To compile only the one-click example:

```bash
cargo build -p create-an-app-in-one-click-rust --locked
```

To run the real flow:

```bash
cd examples/create-an-app-in-one-click-rust
cp .env.example .env
cargo run --locked
```

## 12. Release gate

Before tagging a release:

- [ ] all commands in section 3 pass on a clean checkout;
- [ ] GitHub Actions pass on Linux, macOS, and Windows;
- [ ] `cargo package --list` contains only intended files;
- [ ] README code and feature names match the public API;
- [ ] no credentials exist in Git history;
- [ ] dependency advisory scan passes or exceptions are documented;
- [ ] version, changelog, tag, and crate metadata agree;
- [ ] at least one controlled live registration test is completed;
- [ ] the returned App Secret is rotated or the test application is removed after validation.

## CI/CD validation

The repository includes:

- `.github/workflows/ci.yml`: cross-platform tests and release builds, formatting, Clippy with warnings denied, rustdoc with warnings denied, feature-matrix checks, and crate packaging.
- `.github/workflows/release.yml`: tag/manual release verification, multi-platform binary builds, checksum generation, GitHub Release creation, and optional crates.io publishing.

The release workflow only runs on a version tag or an explicitly forced manual dispatch.
