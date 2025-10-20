#!/bin/bash
# DeepSeek-OCR Setup Script for ARM64 + CUDA 13.0
# This script installs all dependencies needed to run DeepSeek-OCR

set -e  # Exit on error

echo "=========================================="
echo "DeepSeek-OCR Setup Script"
echo "Environment: ARM64 (aarch64) + CUDA 13.0"
echo "=========================================="
echo ""

# Check if running in correct directory
if [ ! -d "DeepSeek-OCR" ] || [ ! -d "DeepSeek-OCR-model" ]; then
    echo "ERROR: Please run this script from the deepseek-ocr directory"
    echo "Expected directory structure:"
    echo "  - DeepSeek-OCR/ (code repository)"
    echo "  - DeepSeek-OCR-model/ (model files)"
    exit 1
fi

echo "Step 1: Checking system requirements..."
echo "Python version: $(python3 --version)"
echo "CUDA version: $(nvcc --version | grep release)"
echo "Architecture: $(uname -m)"
echo ""

echo "Step 2: Installing PyTorch for ARM64 + CUDA 13.0..."
echo "Installing PyTorch 2.9.0 with CUDA 13.0 support (required for GB10/sm_121)"
echo "Note: Using --break-system-packages (safe in Docker container)"
# Install PyTorch 2.9.0 with CUDA 13.0 support
# This version works with NVIDIA GB10 (sm_121) despite showing a warning
pip3 install torch torchvision torchaudio --break-system-packages --index-url https://download.pytorch.org/whl/cu130

# Verify PyTorch installation
python3 -c "import torch; print(f'PyTorch {torch.__version__} installed'); print(f'CUDA available: {torch.cuda.is_available()}')" || \
    (echo "ERROR: PyTorch installation verification failed" && exit 1)

echo ""
echo "Step 3: Installing base requirements..."
pip3 install --break-system-packages -r DeepSeek-OCR/requirements.txt

echo ""
echo "Step 4: Installing additional dependencies..."
# Install commonly needed packages
pip3 install --break-system-packages safetensors accelerate sentencepiece

echo ""
echo "Step 5: Attempting to install flash-attention..."
echo "NOTE: This may take several minutes and might fail on ARM64"
echo "If it fails, the model will fall back to standard attention mechanisms"
# Try to install flash-attn, but don't fail if it doesn't work
pip3 install --break-system-packages flash-attn --no-build-isolation || \
    echo "WARNING: flash-attn installation failed. Will use fallback attention mechanism."

echo ""
echo "Step 6: Verifying installations..."
python3 << 'PYEOF'
import sys
print("Checking installed packages...")

required = {
    'torch': 'PyTorch',
    'transformers': 'Transformers',
    'PIL': 'Pillow',
    'numpy': 'NumPy',
    'einops': 'einops',
}

missing = []
for module, name in required.items():
    try:
        __import__(module)
        print(f"✓ {name}")
    except ImportError:
        print(f"✗ {name} - MISSING")
        missing.append(name)

# Check optional packages
optional = {
    'flash_attn': 'Flash Attention',
}

for module, name in optional.items():
    try:
        __import__(module)
        print(f"✓ {name} (optional)")
    except ImportError:
        print(f"○ {name} (optional - not available)")

if missing:
    print(f"\nERROR: Missing required packages: {', '.join(missing)}")
    sys.exit(1)
else:
    print("\n✓ All required packages installed successfully!")
PYEOF

echo ""
echo "=========================================="
echo "Setup completed successfully!"
echo "=========================================="
echo ""
echo "Model location: ./DeepSeek-OCR-model/"
echo "Code location: ./DeepSeek-OCR/"
echo ""
echo "Next steps:"
echo "  1. Download a test image (or run download_test_image.sh)"
echo "  2. Run: bash run_ocr.sh"
echo ""
