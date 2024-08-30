#!/bin/bash
set -e

# ensure permission to write to /usr/local/bin
if [ ! -w /usr/local/bin ]; then
    echo "Error: You don't have write permission to /usr/local/bin"
    echo "Try running this script with sudo"
    exit 1
fi

# Build and install each crate
for crate in melond mbatch mworker mqueue mcancel mextend mshow; do
    echo "Building $crate..."
    cargo build --release --manifest-path crates/$crate/Cargo.toml

    echo "Installing $crate to /usr/local/bin..."
    cp target/release/$crate /usr/local/bin/
    chmod +x /usr/local/bin/$crate
done

echo "All binaries installed successfully in /usr/local/bin!"
