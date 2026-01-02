#!/bin/bash
# Install dependencies for Rust GUI

echo "Installing GTK4 and libadwaita development libraries..."
echo "This requires sudo access."
echo ""

sudo apt install -y libgtk-4-dev libadwaita-1-dev pkg-config

if [[ $? -eq 0 ]]; then
    echo "Dependencies installed successfully!"
else
    echo "Failed to install dependencies"
    exit 1
fi
