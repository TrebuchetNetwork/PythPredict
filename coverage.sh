#!/bin/bash

# Install tarpaulin if not installed
cargo install cargo-tarpaulin

# Run coverage for Rust unit tests
cargo tarpaulin --workspace \
    --exclude-files "*/target/*" \
    --exclude-files "*/tests/*" \
    --ignore-panics \
    --ignore-tests \
    --out Html \
    --out Lcov \
    --output-dir ./coverage

echo "Coverage report generated in ./coverage/tarpaulin-report.html"