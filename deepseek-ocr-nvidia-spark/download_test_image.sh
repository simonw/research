#!/bin/bash
# Download test image for OCR testing

set -e

echo "Downloading test image from simonwillison.net..."
curl -L -o test_image.jpeg https://static.simonwillison.net/static/2025/ft.jpeg

if [ -f "test_image.jpeg" ]; then
    echo "✓ Test image downloaded successfully: test_image.jpeg"
    file test_image.jpeg
    ls -lh test_image.jpeg
else
    echo "ERROR: Failed to download test image"
    exit 1
fi
