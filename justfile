# Common tarpaulin flags
tarpaulin_base := "tarpaulin --workspace --all-features --timeout 120 --exclude-files src/main.rs --ignore-tests --target-dir $PWD/target-cov --skip-clean"

# Full coverage report with HTML and XML output
coverage:
    cargo {{tarpaulin_base}} --out html --out xml --output-dir coverage

# Quick coverage check (just show percentage)
coverage-check:
    cargo {{tarpaulin_base}}

# Coverage with lcov output
coverage-lcov:
    cargo {{tarpaulin_base}} --out lcov

# Strict Clippy check
strict:
    cargo clippy --workspace --all-targets --all-features -- -D warnings