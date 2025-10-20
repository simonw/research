# ✅ SOLUTION: DeepSeek-OCR Working on NVIDIA GB10 (sm_121)

## The Problem

NVIDIA GB10 GPU has CUDA compute capability **sm_121**, which was not supported by PyTorch 2.5.1:
- PyTorch 2.5.1 supported: sm_50, sm_80, sm_86, sm_89, sm_90, sm_90a
- GB10 requires: sm_121
- Result: `CUDA error: no kernel image is available for execution on the device`

## The Solution

**Use PyTorch 2.9.0 with CUDA 13.0 wheels!**

### Quick Fix

```bash
# Uninstall old PyTorch
pip3 uninstall -y --break-system-packages torch torchvision torchaudio

# Install PyTorch 2.9.0 with CUDA 13.0
pip3 install --break-system-packages torch torchvision torchaudio \
    --index-url https://download.pytorch.org/whl/cu130
```

### Why This Works

1. **PyTorch 2.9.0 has ARM64 wheels for CUDA 13.0** (`cu130`)
2. **Better forward compatibility**: sm_121 works despite being slightly above the official max (12.0)
3. **Native CUDA 13.0 support**: Matches your system's CUDA version perfectly
4. **Graceful degradation**: Uses sm_120 kernels which are compatible with sm_121

### Verification

```bash
python3 -c "import torch; \
    print(f'PyTorch: {torch.__version__}'); \
    print(f'CUDA available: {torch.cuda.is_available()}'); \
    print(f'CUDA version: {torch.version.cuda}'); \
    print(f'Device: {torch.cuda.get_device_name(0)}')"
```

Expected output:
```
PyTorch: 2.9.0+cu130
CUDA available: True
CUDA version: 13.0
Device: NVIDIA GB10
```

You'll see a warning about sm_121 being above the max supported (12.0), but **it works anyway!**

## Test Results

### Performance
- **Image**: 3503×1668 pixels (586KB)
- **Model loading**: 33.82 seconds
- **Inference time**: 58.27 seconds
- **Total time**: ~92 seconds

### OCR Output
- Detected **2257 text tokens** from **921 vision tokens**
- Compression ratio: 2.45x
- Generated bounding boxes for all detected text
- Created annotated image with text locations
- Successfully extracted text from multi-column article layout

### Output Files
```
output/
├── result_with_boxes.jpg    # 976KB - Image with bounding boxes
├── result.mmd                # OCR text output
└── images/                   # Intermediate processing
```

## Complete Working Setup

```bash
cd deepseek-ocr

# 1. Run setup (if not already done)
bash setup.sh

# 2. Upgrade to PyTorch 2.9.0
pip3 uninstall -y --break-system-packages torch torchvision torchaudio
pip3 install --break-system-packages torch torchvision torchaudio \
    --index-url https://download.pytorch.org/whl/cu130

# 3. Download test image (if not already done)
bash download_test_image.sh

# 4. Run OCR
python3 run_ocr.py
```

## Key Learnings

### 1. Check Multiple PyTorch Versions
Don't assume the latest stable is your only option. Check for:
- Different CUDA versions (cu118, cu124, cu130, etc.)
- Pre-release/nightly builds
- Platform-specific wheels (ARM64 vs x86_64)

### 2. CUDA Version Matching Matters
Matching your system's CUDA version (13.0) with PyTorch's CUDA support (cu130) provides better compatibility.

### 3. Warnings vs Errors
PyTorch 2.9.0 warns about sm_121 but continues execution. Earlier versions hard-failed.

### 4. Check Official Wheels Repository
```bash
# List available PyTorch wheels for your platform
curl -s https://download.pytorch.org/whl/torch/ | grep aarch64 | grep cu130
```

## Available PyTorch CUDA Versions for ARM64

PyTorch 2.9.0 has ARM64 wheels for:
- **cu128** (CUDA 12.8)
- **cu129** (CUDA 12.9)
- **cu130** (CUDA 13.0) ✅ Our solution
- Earlier: cu124, cu121, cu118, etc.

## Updated Setup Script

The `setup.sh` has been updated to use PyTorch 2.9.0+cu130 by default. For new installations:

```bash
bash setup.sh  # Now installs PyTorch 2.9.0 automatically
```

## Comparison: Before vs After

| Aspect | PyTorch 2.5.1 | PyTorch 2.9.0 |
|--------|---------------|---------------|
| CUDA Support | cu124 | cu130 ✅ |
| sm_121 Status | Hard Error ❌ | Warning but Works ✅ |
| Inference | Failed | Success ✅ |
| Installation | Required workarounds | Clean install ✅ |

## Recommendation

**For NVIDIA GB10 (sm_121) or any new NVIDIA GPU:**
1. Start with the latest PyTorch version
2. Match CUDA versions exactly (system CUDA 13.0 → cu130)
3. Try even if officially "unsupported" - forward compatibility often works
4. Upgrade PyTorch before attempting workarounds

## References

- [PyTorch CUDA 13.0 Wheels](https://download.pytorch.org/whl/cu130/)
- [PyTorch Installation Guide](https://pytorch.org/get-started/locally/)
- [DeepSeek-OCR GitHub](https://github.com/deepseek-ai/DeepSeek-OCR)
- [DeepSeek-OCR Model](https://huggingface.co/deepseek-ai/DeepSeek-OCR)

---

**Status**: ✅ **FULLY WORKING**
**Date**: 2025-10-20
**Platform**: NVIDIA GB10 (ARM64) + CUDA 13.0
**Solution**: PyTorch 2.9.0+cu130
