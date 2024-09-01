#!/bin/bash
set -e

for crate in melond mbatch mqueue mcancel mextend mshow; do
    echo "Building $crate..."
    cargo build --release --manifest-path crates/$crate/Cargo.toml
done

cargo build --release --manifest-path crates/mworker/Cargo.toml --features cgroups

echo "Installing binaries to /usr/local/bin. You may be prompted for your password."
for crate in melond mbatch mworker mqueue mcancel mextend mshow; do
    echo "Installing $crate to /usr/local/bin..."
    sudo cp target/release/$crate /usr/local/bin/
    sudo chmod +x /usr/local/bin/$crate
done

echo "All binaries installed successfully in /usr/local/bin!"
