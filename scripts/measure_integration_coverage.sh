#!/bin/bash

# Build with coverage instrumentation
echo "Building with coverage instrumentation..."
cargo build-bpf -- --features coverage

# Run integration tests with coverage collection
echo "Running integration tests..."
RUST_LOG=solana_runtime::system_instruction_processor=trace \
COVERAGE_DIR=./target/coverage \
npm test

# Process coverage data
echo "Processing coverage data..."
cargo profdata -- merge -sparse \
    ./target/coverage/default.profraw \
    -o ./target/coverage/tests.profdata

# Generate report
cargo cov -- report \
    --use-color \
    --ignore-filename-regex='/.cargo/|/rustc/' \
    --instr-profile=./target/coverage/tests.profdata \
    --object ./target/deploy/pythpredict.so

# Generate HTML report
cargo cov -- show \
    --use-color \
    --ignore-filename-regex='/.cargo/|/rustc/' \
    --instr-profile=./target/coverage/tests.profdata \
    --object ./target/deploy/pythpredict.so \
    --show-instantiations \
    --show-line-counts-or-regions \
    --output-dir=./coverage/integration \
    --format=html

echo "Coverage report available at ./coverage/integration/index.html"