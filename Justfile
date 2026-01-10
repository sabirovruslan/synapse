default:
    @just --list

build:
    cargo build --workspace --exclude synapse-py

test:
    cargo test --workspace --exclude synapse-py

bench:
    cargo bench
