# DeepSeek-OCR Setup on ARM64 + CUDA 13.0

## Project Overview

This project documents the setup and testing of [DeepSeek-OCR](https://github.com/deepseek-ai/DeepSeek-OCR), a state-of-the-art optical character recognition model, on an NVIDIA ARM64 device with CUDA 13.0.

**Status**: ⚠️ **Partially Successful** - Setup completed but inference blocked by hardware compatibility

## Table of Contents

- [Environment](#environment)
- [What Was Accomplished](#what-was-accomplished)
- [Key Challenge](#key-challenge)
- [Installation](#installation)
- [Usage](#usage)
- [Technical Details](#technical-details)
- [Lessons Learned](#lessons-learned)
- [Future Solutions](#future-solutions)
- [Files in This Repository](#files-in-this-repository)

## Environment

### Hardware & System
- **Architecture**: ARM64 (aarch64)
- **GPU**: NVIDIA GB10 (CUDA Compute Capability sm_121)
- **CUDA Version**: 13.0
- **Driver Version**: 580.82.09
- **RAM**: 119GB
- **Disk Space**: 3.2TB available
- **OS**: Linux 6.11.0-1014-nvidia

### Software
- **Python**: 3.12.3
- **PyTorch**: 2.5.1 (with CUDA 12.4 support)
- **Transformers**: 4.46.3
- **Model Size**: 6.3GB (DeepSeek-OCR safetensors)

## What Was Accomplished

✅ **Successfully Completed:**

1. **Environment Setup**
   - Installed pip, git-lfs, and build tools
   - Configured Git LFS for large file handling

2. **Repository Cloning**
   - Cloned DeepSeek-OCR code repository
   - Cloned 6.3GB model from HuggingFace with LFS

3. **Dependency Installation**
   - PyTorch 2.5.1 with CUDA 12.4 support
   - Transformers 4.46.3 with all dependencies
   - Document processing libraries (PyMuPDF, img2pdf)
   - Utility libraries (einops, accelerate, safetensors)

4. **Model Loading**
   - Model loaded successfully in ~34 seconds
   - Tokenizer initialized correctly
   - Vision encoder and language model components loaded

5. **Image Preprocessing**
   - Test image downloaded (586KB JPEG)
   - Image preprocessing pipeline functional
   - Image resizing and formatting works correctly

6. **Script Creation**
   - `setup.sh` - Automated dependency installation
   - `download_test_image.sh` - Test image downloader
   - `run_ocr.py` - GPU inference script
   - `run_ocr_cpu_nocuda.py` - CPU fallback script

## Key Challenge

### The CUDA Compute Capability Issue

**Problem**: The NVIDIA GB10 GPU has CUDA compute capability **sm_121**, which is not supported by PyTorch 2.5.1.

**PyTorch 2.5.1 Supports**:
- sm_50 (Maxwell)
- sm_80 (Ampere)
- sm_86 (Ampere)
- sm_89 (Ada Lovelace)
- sm_90 (Hopper)
- sm_90a (Hopper)

**GB10 Requires**: sm_121 (newer architecture)

### Error Message
```
NVIDIA GB10 with CUDA capability sm_121 is not compatible with the current PyTorch installation.
RuntimeError: CUDA error: no kernel image is available for execution on the device
```

### CPU Fallback Blocked

Attempting to run on CPU revealed a second issue:
- The model has hardcoded `.cuda()` calls in its implementation
- Located at `modeling_deepseekocr.py:917`
- No CPU fallback mechanism in the inference code

## Installation

### Prerequisites
- ARM64 Linux system
- Python 3.12+
- Git and Git LFS
- NVIDIA GPU with supported compute capability (or plan to use CPU after code modifications)

### Quick Start

```bash
# 1. Clone this project (or navigate to the deepseek-ocr directory)
cd deepseek-ocr

# 2. Run the setup script
bash setup.sh

# 3. Download test image
bash download_test_image.sh

# 4. Attempt inference (will fail on unsupported GPUs)
python3 run_ocr.py
```

### What Each Script Does

**`setup.sh`**
- Installs PyTorch with CUDA support
- Installs all required dependencies
- Attempts flash-attention (fails gracefully on ARM64)
- Verifies all installations

**`download_test_image.sh`**
- Downloads test image from simonwillison.net
- Verifies download success

**`run_ocr.py`**
- Main inference script for GPU execution
- Loads model and tokenizer
- Processes images with OCR
- Saves results to output directory

**`run_ocr_cpu_nocuda.py`**
- CPU-only version (currently blocked by model code)
- Disables CUDA completely
- Would work if model code is modified

## Usage

### If You Have a Supported GPU

```bash
python3 run_ocr.py
```

The script will:
1. Load the model (~34 seconds)
2. Process your image
3. Output OCR results
4. Save results to `./output/` directory

### Supported Image Modes

The model supports several prompts for different use cases:

```python
# Document OCR (markdown output)
prompt = "<image>\n<|grounding|>Convert the document to markdown."

# General OCR
prompt = "<image>\n<|grounding|>OCR this image."

# Free-form OCR
prompt = "<image>\nFree OCR."

# Detailed description
prompt = "<image>\nDescribe this image in detail."
```

### Image Size Modes

```python
# Tiny: 512×512 (64 vision tokens)
base_size=512, image_size=512, crop_mode=False

# Small: 640×640 (100 vision tokens)
base_size=640, image_size=640, crop_mode=False

# Base: 1024×1024 (256 vision tokens)
base_size=1024, image_size=1024, crop_mode=False

# Large: 1280×1280 (400 vision tokens)
base_size=1280, image_size=1280, crop_mode=False

# Gundam (dynamic): multiple 640×640 + one 1024×1024
base_size=1024, image_size=640, crop_mode=True
```

## Technical Details

### Dependencies Installed

**Core ML Stack:**
```
torch==2.5.1
torchvision==0.20.1
torchaudio==2.5.1
transformers==4.46.3
tokenizers==0.20.3
```

**OCR/Document Processing:**
```
PyMuPDF==1.26.5
img2pdf==0.6.1
pillow==11.3.0
```

**Utilities:**
```
safetensors==0.6.2
accelerate==1.11.0
sentencepiece==0.2.1
einops==0.8.1
numpy==2.3.3
```

**Attempted (Failed on ARM64):**
```
flash-attn==2.8.3  # Compilation failed, model falls back to eager attention
```

### Model Architecture

- **Base Model**: DeepSeek v2 architecture
- **Vision Encoder**: Custom "deepencoder" for OCR
- **Model Type**: Vision-Language Model (VLM)
- **Attention**: Flash Attention 2 (fallback to eager on ARM64)
- **Precision**: bfloat16 (GPU) or float32 (CPU)

### Repository Structure

```
deepseek-ocr/
├── DeepSeek-OCR/              # Code repository
│   ├── DeepSeek-OCR-master/
│   │   ├── DeepSeek-OCR-hf/   # HuggingFace implementation
│   │   └── DeepSeek-OCR-vllm/ # vLLM implementation
│   └── requirements.txt
├── DeepSeek-OCR-model/        # Model files (6.3GB)
│   ├── config.json
│   ├── modeling_deepseekocr.py
│   ├── model-00001-of-000001.safetensors
│   └── tokenizer files
├── test_image.jpeg            # Test image (586KB)
├── output/                    # OCR results directory
├── notes.md                   # Detailed setup notes
├── setup.sh                   # Setup script
├── download_test_image.sh     # Image download script
├── run_ocr.py                 # GPU inference script
├── run_ocr_cpu_nocuda.py      # CPU inference script
└── README.md                  # This file
```

## Lessons Learned

### 1. Hardware Compatibility is Critical

**Finding**: Cutting-edge hardware (GB10 with sm_121) can be *too new* for current software.

**Impact**:
- PyTorch releases lag behind newest GPU architectures
- Always check compute capability before setting up ML environments
- Consider using Docker images with specific PyTorch/CUDA versions

### 2. ARM64 Has Limited ML Ecosystem Support

**Observations**:
- PyTorch ARM64 wheels exist but are less tested
- Flash Attention doesn't build easily on ARM64
- Many ML libraries assume x86_64 architecture

**Workarounds**:
- Fallback attention mechanisms (eager, sdpa)
- Accepting slower performance on ARM64
- Using pre-built Docker images when available

### 3. Model Portability Assumptions

**Issue**: DeepSeek-OCR assumes CUDA availability
- Hardcoded `.cuda()` calls in model code
- No device-agnostic implementation
- CPU inference not supported without code changes

**Best Practice**: Models should use:
```python
device = next(model.parameters()).device
tensor = tensor.to(device)  # Instead of tensor.cuda()
```

### 4. Large Model Considerations

**Model Size**: 6.3GB requires:
- Git LFS for cloning
- Sufficient RAM for loading (8GB+ recommended)
- Fast storage for quick loading times
- Network bandwidth for initial download

### 5. Dependency Management Complexity

**Challenges**:
- Python 3.12's externally-managed-environment
- Conflicting package versions
- Platform-specific wheels (ARM64 vs x86_64)

**Solutions**:
- `--break-system-packages` in Docker containers
- Virtual environments for production
- Careful version pinning

## Future Solutions

### Option 1: Wait for PyTorch Update ⏳

**Timeline**: Likely PyTorch 2.6+ or 3.0

**Action Items**:
1. Monitor [PyTorch releases](https://github.com/pytorch/pytorch/releases)
2. Check for sm_121 support announcements
3. Re-run setup once available
4. Test inference immediately

**Probability**: High (PyTorch typically adds new architectures)

### Option 2: Build PyTorch from Source 🔨

**Difficulty**: High | **Time**: 4-8 hours on ARM64

**Steps**:
```bash
# 1. Install build dependencies
apt-get install cmake ninja-build

# 2. Clone PyTorch
git clone --recursive https://github.com/pytorch/pytorch
cd pytorch

# 3. Configure with sm_121 support
export TORCH_CUDA_ARCH_LIST="8.0;8.6;8.9;9.0;12.1"
export CMAKE_BUILD_TYPE=Release

# 4. Build (takes hours on ARM64)
python setup.py install
```

**Resources Required**:
- 32GB+ RAM
- 50GB+ disk space
- 4-8 hours build time

### Option 3: Modify Model Code 📝

**Difficulty**: Medium | **Time**: 1-2 hours

**Required Changes**:

1. Fork/modify `modeling_deepseekocr.py`
2. Replace hardcoded CUDA calls:

```python
# Before:
input_ids.unsqueeze(0).cuda()

# After:
device = next(self.parameters()).device
input_ids.unsqueeze(0).to(device)
```

3. Test on CPU
4. Submit PR to upstream repository

### Option 4: Use Compatible Hardware 🖥️

**Immediate Solution**: Test on supported GPU

**Supported GPUs**:
- NVIDIA A100 (sm_80)
- NVIDIA A6000 (sm_86)
- NVIDIA RTX 4090 (sm_89)
- NVIDIA H100 (sm_90)

**Platforms**:
- Cloud providers (AWS, GCP, Azure)
- Local workstations
- HuggingFace Inference API

### Option 5: Use Docker with Specific PyTorch Build 🐳

**If Available**: Check for NVIDIA NGC containers with sm_121 support

```bash
# Example (if available)
docker pull nvcr.io/nvidia/pytorch:xx.xx-py3-sm121
```

## Files in This Repository

| File | Purpose | Status |
|------|---------|--------|
| `setup.sh` | Install all dependencies | ✅ Working |
| `download_test_image.sh` | Download test image | ✅ Working |
| `run_ocr.py` | GPU inference script | ⚠️ Blocked by GPU compatibility |
| `run_ocr_cpu_nocuda.py` | CPU fallback script | ⚠️ Blocked by model code |
| `notes.md` | Detailed setup notes | ✅ Complete |
| `README.md` | This documentation | ✅ Complete |
| `test_image.jpeg` | Test image (586KB) | ✅ Downloaded |
| `DeepSeek-OCR/` | Code repository | ✅ Cloned |
| `DeepSeek-OCR-model/` | Model files (6.3GB) | ✅ Cloned |

## Recommendations

### For This Specific Environment (GB10 + CUDA 13.0)

1. **Monitor PyTorch releases** for sm_121 support (most practical)
2. **Consider building PyTorch from source** if urgent (time-intensive)
3. **Modify model code** for CPU compatibility (enables CPU testing)

### For General ML on ARM64

1. **Use Docker images** from NVIDIA NGC when available
2. **Test hardware compatibility** before large setups
3. **Have fallback plans** (CPU, different hardware, cloud)
4. **Budget extra time** for ARM64-specific issues

### For DeepSeek-OCR Specifically

1. **Prefer x86_64 with supported NVIDIA GPUs** (A100, H100, etc.)
2. **Use HuggingFace Inference API** for testing without setup
3. **Contribute CPU-compatible code** to make the model more portable

## Conclusion

This project successfully demonstrated the complete setup process for DeepSeek-OCR on an ARM64 + CUDA 13.0 system, including:

- ✅ Environment configuration
- ✅ Dependency installation
- ✅ Repository and model cloning
- ✅ Model loading and initialization
- ⚠️ Inference blocked by hardware compatibility

The primary blocker—PyTorch lacking sm_121 support—is temporary and will likely be resolved in future PyTorch releases. All setup work is complete and ready for testing once compatible PyTorch becomes available.

### Key Takeaway

> **Hardware compatibility should be verified BEFORE major ML infrastructure setup**. While this project encountered a blocker, the systematic approach and comprehensive documentation ensure that the setup can be quickly resumed once PyTorch adds sm_121 support.

---

## Additional Resources

- [DeepSeek-OCR GitHub](https://github.com/deepseek-ai/DeepSeek-OCR)
- [DeepSeek-OCR on HuggingFace](https://huggingface.co/deepseek-ai/DeepSeek-OCR)
- [DeepSeek-OCR Paper](./DeepSeek-OCR/DeepSeek_OCR_paper.pdf)
- [PyTorch Installation Guide](https://pytorch.org/get-started/locally/)
- [NVIDIA CUDA Compatibility](https://docs.nvidia.com/cuda/cuda-c-programming-guide/index.html#compute-capabilities)

## Contact & Contributions

This setup was created as a learning exercise and documentation effort. Contributions, corrections, and updates are welcome!

For issues specific to:
- **DeepSeek-OCR**: https://github.com/deepseek-ai/DeepSeek-OCR/issues
- **PyTorch**: https://github.com/pytorch/pytorch/issues
- **This setup**: Document in your own notes or share experiences

---

**Last Updated**: 2025-10-20
**Environment**: NVIDIA GB10 (ARM64) + CUDA 13.0 + Docker
**Status**: Setup Complete, Awaiting PyTorch sm_121 Support
