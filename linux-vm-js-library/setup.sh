#!/bin/bash

# Setup script for Linux VM JS Library
# Downloads v86 and necessary Linux images

set -e

echo "Setting up Linux VM JS Library..."

# Create assets directory
mkdir -p assets

# Download v86 library files from jsdelivr CDN (reliable source for npm packages)
echo "Downloading v86 library..."
# Using jsdelivr.net which serves npm packages
curl -L "https://cdn.jsdelivr.net/npm/v86@latest/build/libv86.js" -o assets/libv86.js
curl -L "https://cdn.jsdelivr.net/npm/v86@latest/build/v86.wasm" -o assets/v86.wasm

# Download a minimal Linux image (buildroot)
echo "Downloading Linux images..."
IMAGES_BASE="https://github.com/copy/images/raw/master"

# Download buildroot bzImage (Linux kernel)
curl -L "${IMAGES_BASE}/buildroot/bzImage" -o assets/bzImage

# Download buildroot filesystem
curl -L "${IMAGES_BASE}/buildroot/buildroot-bzimage.bin" -o assets/buildroot.bin

echo "Download complete!"
echo "Assets saved to ./assets/"
echo ""
echo "Note: The assets directory is gitignored to keep repository size small."
