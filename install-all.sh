#!/bin/bash
set -e

for crate in melond mbatch mworker mqueue mcancel mextend; do
    echo "Installing $crate..."
    cargo install --path crates/$crate --force
done

echo "All binaries installed successfully!"
