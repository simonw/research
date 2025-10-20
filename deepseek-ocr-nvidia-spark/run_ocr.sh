#!/bin/bash
# Wrapper script to run DeepSeek-OCR inference

set -e

echo "DeepSeek-OCR Runner"
echo ""

# Check if test image exists
if [ ! -f "test_image.jpeg" ]; then
    echo "Test image not found. Downloading..."
    bash download_test_image.sh
    echo ""
fi

# Run the Python script
python3 run_ocr.py
