# Upgrading to PyTorch 2.9.0 for GB10 Support

If you already ran `setup.sh` with PyTorch 2.5.1 and need to upgrade:

## Quick Upgrade

```bash
cd /deepseek-ocr

# Uninstall old PyTorch
pip3 uninstall -y --break-system-packages torch torchvision torchaudio

# Install PyTorch 2.9.0 with CUDA 13.0
pip3 install --break-system-packages torch torchvision torchaudio \
    --index-url https://download.pytorch.org/whl/cu130

# Verify installation
python3 -c "import torch; print(f'PyTorch: {torch.__version__}'); print(f'CUDA: {torch.cuda.is_available()}')"
```

## Test OCR

```bash
python3 run_ocr.py
```

You'll see a warning about sm_121 being above the max supported (12.0), but it will work!

## Expected Output

```
PyTorch: 2.9.0+cu130
CUDA available: True
CUDA device: NVIDIA GB10
CUDA version: 13.0

[Warning about sm_121 - this is normal and OK]

✓ Model loaded in ~34 seconds
✓ Inference completed in ~58 seconds
```

Results will be saved to `output/` directory.
