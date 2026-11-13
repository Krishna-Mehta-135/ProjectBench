# Verification workflow

Run these checks before opening a pull request:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cd apps/web && npm ci && npm run build
```

CI performs the same checks. It never starts k6 against a configured target and never requires target credentials. A live performance run is a separately reviewed operator action.

