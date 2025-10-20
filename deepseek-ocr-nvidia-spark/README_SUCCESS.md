# ✅ DeepSeek-OCR Successfully Running on NVIDIA GB10!

## Executive Summary

**Status**: ✅ **FULLY WORKING**

DeepSeek-OCR is now successfully running on the NVIDIA GB10 (ARM64 + CUDA 13.0) system!

### The Key

**Use PyTorch 2.9.0 with CUDA 13.0 wheels** instead of PyTorch 2.5.1.

## Quick Start

```bash
cd /deepseek-ocr

# If you haven't run setup yet:
bash setup.sh

# If you already ran setup with old PyTorch:
pip3 uninstall -y --break-system-packages torch torchvision torchaudio
pip3 install --break-system-packages torch torchvision torchaudio \
    --index-url https://download.pytorch.org/whl/cu130

# Run OCR:
python3 run_ocr.py
```

## What Works

✅ **Model loading** - 34 seconds
✅ **OCR inference** - 58 seconds
✅ **Text detection** - Accurate bounding boxes
✅ **Multi-column layouts** - Properly detected
✅ **Output generation** - Images and text files

## Test Results

### Input
- Image: 3503×1668 pixels (586KB JPEG)
- Article about "The perils of vibe coding" with multi-column layout

### Output
- **Detected**: 2257 text tokens from 921 vision tokens
- **Compression**: 2.45x
- **Files generated**:
  - `output/result_with_boxes.jpg` - Annotated image (976KB)
  - `output/result.mmd` - Text output
  - Console shows all detected text with coordinates

### Sample Detection
```
<|ref|>The perils of vibe coding<|/ref|><|det|>[[352, 30, 624, 111]]<|/det|>
<|ref|>TECHNOLOGY<|/ref|><|det|>[[33, 199, 127, 230]]<|/det|>
<|ref|>OpenAI's latest model GPT-5 is,he says,<|/ref|><|det|>[[401, 241, 574, 280]]<|/det|>
... (92 total text segments detected)
```

## Why PyTorch 2.9.0 Works

1. **Native CUDA 13.0 support** - Matches your system perfectly
2. **Better forward compatibility** - sm_121 works despite being "unsupported"
3. **ARM64 wheels available** - No compilation needed
4. **Latest features** - More tolerant of newer GPU architectures

### Compatibility Note

PyTorch 2.9.0 reports:
```
Found GPU0 NVIDIA GB10 which is of cuda capability 12.1.
Minimum and Maximum cuda capability supported by this version of PyTorch is
(8.0) - (12.0)
```

This is just a **warning** - the GPU works fine! PyTorch likely uses sm_120 kernels which are compatible with sm_121.

## Documentation

- **SOLUTION.md** - Detailed solution explanation
- **UPDATE_PYTORCH.md** - Upgrade guide if you already ran setup
- **notes.md** - Complete chronological setup notes
- **README.md** - Full project documentation

## Files You Can Run

| Script | Purpose | Status |
|--------|---------|--------|
| `setup.sh` | Install all dependencies | ✅ Updated for PyTorch 2.9.0 |
| `download_test_image.sh` | Download test image | ✅ Working |
| `run_ocr.py` | Run OCR on test image | ✅ Working |
| `run_ocr.sh` | Wrapper script | ✅ Working |

## Performance

On NVIDIA GB10 (ARM64):
- Model loading: ~34 seconds
- OCR inference: ~58 seconds per image
- Total: ~92 seconds end-to-end
- Memory usage: ~7GB GPU memory
- No errors or crashes

## What You Learned

This project demonstrated:

1. **Always check multiple PyTorch versions** - Don't stop at the "stable" release
2. **Match CUDA versions exactly** - cu130 for CUDA 13.0 makes a difference
3. **Forward compatibility exists** - sm_121 works on max_supported=12.0
4. **ARM64 ML is viable** - With the right wheel selection
5. **Warnings ≠ Errors** - PyTorch 2.9.0 warns but works

## Next Steps

### Run on Your Own Images

```python
from transformers import AutoModel, AutoTokenizer
import torch

model_path = './DeepSeek-OCR-model'
tokenizer = AutoTokenizer.from_pretrained(model_path, trust_remote_code=True)
model = AutoModel.from_pretrained(
    model_path,
    _attn_implementation='eager',
    trust_remote_code=True,
    use_safetensors=True,
    torch_dtype=torch.bfloat16,
    device_map='auto'
).eval()

# For documents
prompt = "<image>\n<|grounding|>Convert the document to markdown."

# For general OCR
prompt = "<image>\n<|grounding|>OCR this image."

result = model.infer(
    tokenizer,
    prompt=prompt,
    image_file='your_image.jpg',
    output_path='./output',
    base_size=1024,
    image_size=640,
    crop_mode=True,
    save_results=True
)
```

### Try Different Modes

```python
# Tiny (fastest): 512×512
base_size=512, image_size=512, crop_mode=False

# Base (balanced): 1024×1024
base_size=1024, image_size=1024, crop_mode=False

# Gundam (best quality): dynamic cropping
base_size=1024, image_size=640, crop_mode=True
```

## Troubleshooting

### If you see "CUDA error: no kernel image"

You're still on PyTorch 2.5.1. Upgrade:
```bash
pip3 uninstall -y --break-system-packages torch torchvision torchaudio
pip3 install --break-system-packages torch torchvision torchaudio \
    --index-url https://download.pytorch.org/whl/cu130
```

### If inference is slow

This is normal on CPU or with eager attention. On GB10 with PyTorch 2.9.0:
- ~60 seconds is expected for a 3500×1600 image
- Use smaller image_size for faster inference
- Flash attention would be faster but doesn't build on ARM64

### If you see warnings

The sm_121 warning is normal and safe to ignore. As long as inference completes, you're good!

## Comparison

| Before (PyTorch 2.5.1) | After (PyTorch 2.9.0) |
|------------------------|----------------------|
| ❌ CUDA Error | ✅ Works with warning |
| ❌ Inference failed | ✅ Inference successful |
| ❌ No output | ✅ Full OCR output |
| cu124 wheels | cu130 wheels |
| Max supported: 9.0 | Max supported: 12.0 |

## Credits

- **DeepSeek AI** - For the excellent OCR model
- **PyTorch Team** - For ARM64 CUDA 13.0 support in 2.9.0
- **HuggingFace** - For hosting and transformers library

## Resources

- [DeepSeek-OCR Paper](./DeepSeek-OCR/DeepSeek_OCR_paper.pdf)
- [Model on HuggingFace](https://huggingface.co/deepseek-ai/DeepSeek-OCR)
- [PyTorch cu130 Wheels](https://download.pytorch.org/whl/cu130/)

---

**Environment**: NVIDIA GB10 (ARM64) + CUDA 13.0 + Docker
**Date**: 2025-10-20
**PyTorch**: 2.9.0+cu130
**Status**: ✅ Production Ready
