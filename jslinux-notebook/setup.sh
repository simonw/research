#!/bin/bash
# Setup script for JSLinux Notebook
# Downloads and extracts JSLinux files from bellard.org

set -e

echo "Setting up JSLinux Notebook..."

# Download JSLinux if not already present
if [ ! -f "jslinux-2019-12-21.tar.gz" ]; then
    echo "Downloading JSLinux package..."
    curl -o jslinux-2019-12-21.tar.gz https://bellard.org/tinyemu/jslinux-2019-12-21.tar.gz
else
    echo "JSLinux package already downloaded."
fi

# Extract if not already extracted
if [ ! -d "jslinux/jslinux-2019-12-21" ]; then
    echo "Extracting JSLinux..."
    mkdir -p jslinux
    tar -xzf jslinux-2019-12-21.tar.gz -C jslinux
    echo "Extracted to jslinux/jslinux-2019-12-21/"
else
    echo "JSLinux already extracted."
fi

echo ""
echo "✅ Setup complete!"
echo ""
echo "To run the notebook:"
echo "  1. python -m http.server 8888"
echo "  2. Open http://localhost:8888/notebook.html in your browser"
echo ""
