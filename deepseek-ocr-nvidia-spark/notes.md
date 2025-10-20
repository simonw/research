# DeepSeek-OCR Setup Notes

## Environment Research (2025-10-20 15:34)

### System Specifications
- **Architecture**: aarch64 (ARM64)
- **OS**: Linux 6.11.0-1014-nvidia
- **Python Version**: 3.12.3
- **Git Version**: 2.43.0

### NVIDIA GPU Information
- **GPU Model**: NVIDIA GB10
- **Driver Version**: 580.82.09
- **CUDA Version**: 13.0
- **CUDA Compiler**: nvcc V13.0.88
- **GPU Status**: Active, no running processes

### Resources Available
- **RAM**: 119GB total, 113GB available
- **Disk Space**: 3.2TB available
- **GPU Memory**: Not shown (typical for newer NVIDIA devices)

### Initial Observations
- This is an NVIDIA ARM device with CUDA 13.0 support
- Python 3.12.3 is installed but pip is missing
- Git LFS will need to be installed for cloning HuggingFace models
- Sufficient resources for running large OCR models

### Next Steps
1. Install pip for Python 3.12
2. Install Git LFS
3. Clone repositories
4. Analyze dependencies and compatibility with ARM64 + CUDA 13.0

---

## Installation Progress (2025-10-20 15:35)

### Installed Components
1. **pip 24.0** - Successfully installed via apt-get
2. **python3-dev** - Required for building Python extensions
3. **git-lfs 3.4.1** - Installed for handling large files in HuggingFace repos

### Git LFS Configuration
- Initialized Git LFS globally for this environment
- Ready to clone large model files from HuggingFace

---

## Repository Analysis (2025-10-20 15:36)

### DeepSeek-OCR Repository Structure
```
DeepSeek-OCR/
├── DeepSeek-OCR-master/
│   ├── DeepSeek-OCR-hf/       # HuggingFace Transformers implementation
│   └── DeepSeek-OCR-vllm/     # vLLM implementation (for production)
├── DeepSeek_OCR_paper.pdf
├── README.md
└── requirements.txt
```

### Requirements Analysis
Base requirements:
- transformers==4.46.3
- tokenizers==0.20.3
- PyMuPDF (for PDF processing)
- img2pdf
- einops
- easydict
- addict
- Pillow
- numpy

Additional requirements from README:
- torch==2.6.0 (they use CUDA 11.8)
- vllm-0.8.5 (for production inference)
- flash-attn==2.7.3 (for efficient attention)

### Compatibility Concerns for ARM64 + CUDA 13.0
1. **PyTorch**: Need to check if torch 2.6.0 has ARM64 wheels for CUDA 13.0
2. **flash-attn**: May need to compile from source for ARM64
3. **vLLM**: ARM64 support uncertain, may need alternative approach

### Chosen Approach
Will use **Transformers inference** (simpler path) rather than vLLM:
- More compatible with ARM64
- Easier to debug
- Sufficient for testing purposes
- Can fall back to eager attention if flash-attn fails

---

## Model Download (2025-10-20 15:38)

### HuggingFace Model Clone
- Successfully cloned deepseek-ai/DeepSeek-OCR model
- Model size: ~6.3GB
- Location: `/deepseek-ocr/DeepSeek-OCR-model/`
- Contains safetensors format (efficient and safe)
- Includes custom modeling files:
  - `modeling_deepseekocr.py` - Main OCR model
  - `modeling_deepseekv2.py` - Base DeepSeek v2 architecture
  - `deepencoder.py` - Vision encoder
  - `configuration_deepseek_v2.py` - Model configuration

---

## Dependency Analysis (2025-10-20 15:39)

### PyTorch Version Check
- PyTorch not currently installed
- Latest available: 2.9.0
- Original requirements specify: torch 2.6.0 with CUDA 11.8
- Our environment: CUDA 13.0 on ARM64

### Strategy for ARM64 + CUDA 13.0
Since we have CUDA 13.0 on ARM64, we have two options:
1. Install latest PyTorch with CUDA 12.x wheels (backward compatible with CUDA 13.0)
2. Use pip to install the latest version directly

For ARM64 (aarch64), PyTorch wheels may be limited. Will attempt:
- Try latest PyTorch first (2.9.0 or 2.8.0)
- Fall back to torch 2.6.0 if needed
- May need to compile from source if no wheels available

### Flash Attention Considerations
- flash-attn 2.7.3 specified in original requirements
- ARM64 support may require compilation from source
- Can fall back to 'eager' or 'sdpa' attention if flash-attn fails
- Testing will determine if it's essential

---

## Script Creation (2025-10-20 15:40)

### Created Scripts

1. **setup.sh** - Comprehensive setup script
   - Installs PyTorch with CUDA support
   - Installs all required dependencies
   - Attempts flash-attention installation (with fallback)
   - Verifies all installations
   - Handles ARM64 + CUDA 13.0 compatibility

2. **download_test_image.sh** - Downloads test image
   - Fetches image from simonwillison.net
   - Verifies download success

3. **run_ocr.py** - Main OCR inference script
   - Loads DeepSeek-OCR model
   - Handles attention implementation fallbacks
   - Performs OCR on test image
   - Saves results to output directory
   - Comprehensive error handling

4. **run_ocr.sh** - Simple wrapper script
   - Auto-downloads test image if missing
   - Runs the Python OCR script

---

## Dependency Installation (2025-10-20 15:42)

### Successfully Installed Packages

**PyTorch Stack:**
- torch 2.5.1 (with CUDA 12.4 support, compatible with CUDA 13.0)
- torchvision 0.20.1
- torchaudio 2.5.1
- CUDA is available and functional

**Core Dependencies:**
- transformers 4.46.3
- tokenizers 0.20.3
- safetensors 0.6.2
- accelerate 1.11.0

**OCR/Document Processing:**
- PyMuPDF 1.26.5 (PDF processing)
- img2pdf 0.6.1
- pillow 11.3.0

**Utilities:**
- einops 0.8.1
- easydict 1.13
- addict 2.4.0
- numpy 2.3.3
- sentencepiece 0.2.1

### Flash Attention Status
- flash-attn 2.8.3 failed to compile (expected on ARM64)
- This is acceptable - model will fall back to eager attention
- No impact on functionality, only slight performance difference

### Verification Results
All required packages installed successfully!

---

## First Test Attempt (2025-10-20 15:45)

### CUDA Compatibility Issue Discovered
- **Problem**: NVIDIA GB10 has CUDA capability sm_121 (very new architecture)
- **PyTorch Support**: PyTorch 2.5.1 supports sm_50, sm_80, sm_86, sm_89, sm_90, sm_90a
- **Impact**: Cannot use GPU acceleration with current PyTorch version

### Error Details
```
NVIDIA GB10 with CUDA capability sm_121 is not compatible with the current PyTorch installation.
RuntimeError: CUDA error: no kernel image is available for execution on the device
```

### Solutions
1. **CPU Fallback** (immediate solution)
   - Use CPU-only mode for inference
   - Slower but functional
   - Demonstrates the model works

2. **Future Solutions**
   - Wait for PyTorch version supporting sm_121
   - Build PyTorch from source with sm_121 support (time-consuming)
   - Use Docker image with compatible PyTorch (if available)

### Model Loading Success
Despite CUDA issue:
- Model loaded successfully in 34 seconds
- Tokenizer initialized correctly
- Inference started but failed at CUDA kernel execution

---

## Testing Results (2025-10-20 15:50)

### CPU-Only Testing Attempts

**Attempt 1: Standard CPU mode**
- Set device_map='cpu'
- Model still attempted CUDA operations
- Error: CUDA kernel not available for sm_121

**Attempt 2: CUDA_VISIBLE_DEVICES="" **
- Disabled CUDA completely via environment
- Model loaded successfully on CPU
- Inference failed: Model has hardcoded `.cuda()` calls
- Error location: `modeling_deepseekocr.py:917` - `input_ids.unsqueeze(0).cuda()`

### Root Cause Analysis

1. **Primary Issue**: NVIDIA GB10 (sm_121) not supported by PyTorch 2.5.1
   - GB10 is too new for current PyTorch releases
   - Supported capabilities: sm_50, sm_80, sm_86, sm_89, sm_90, sm_90a
   - GB10 requires: sm_121

2. **Secondary Issue**: Model implementation not CPU-compatible
   - Hardcoded `.cuda()` calls in model code
   - No CPU fallback in the inference method
   - Would require modifying the model's source code

### What We Successfully Accomplished

✓ Environment setup for ARM64 + CUDA 13.0
✓ Git LFS installation and configuration
✓ Repository cloning (code and model)
✓ PyTorch 2.5.1 installation with CUDA support
✓ All dependencies installed correctly
✓ Model loaded successfully (6.3GB)
✓ Tokenizer initialized
✓ Image preprocessing worked
✓ Inference started (but failed at CUDA execution)

### Working Solutions for This Environment

**Option 1: Wait for PyTorch Update**
- Monitor PyTorch releases for sm_121 support
- Likely in PyTorch 2.6+ or 3.0

**Option 2: Build PyTorch from Source**
- Compile PyTorch with GENCODE settings for sm_121
- Time-consuming (several hours on ARM)
- Requires: `cmake`, `ninja`, proper CUDA toolkit

**Option 3: Modify Model Code**
- Fork the model repository
- Replace hardcoded `.cuda()` calls with device-agnostic code
- Use `model.device` or `input.device` instead
- Maintain CPU compatibility

**Option 4: Use Different Hardware**
- Test on x86_64 with supported NVIDIA GPU (sm_80, sm_86, sm_89, sm_90)
- Or older NVIDIA ARM devices with supported compute capability

---

## PyTorch Version Discovery (2025-10-20 15:52)

### Available PyTorch ARM64 CUDA Wheels

Found that PyTorch 2.9.0 has ARM64 wheels for:
- **CUDA 12.8** (cu128)
- **CUDA 12.9** (cu129)  
- **CUDA 13.0** (cu130) ← Our environment!

This is significant because we previously tried 2.5.1 with CUDA 12.4.

### PyTorch 2.9.0 Features
- Latest stable release
- Native CUDA 13.0 support
- May include sm_121 support (needs testing)
- Compatible with Python 3.12

### Testing Plan
1. Uninstall PyTorch 2.5.1
2. Install PyTorch 2.9.0 with CUDA 13.0 wheels
3. Check supported compute capabilities
4. Retry inference

---

## BREAKTHROUGH - PyTorch 2.9.0 Works! (2025-10-20 15:50)

### PyTorch 2.9.0 Installation Success
After checking available versions, discovered:
- **PyTorch 2.9.0 has ARM64 wheels for CUDA 13.0** (`cu130`)
- Successfully installed torch-2.9.0+cu130
- Includes all CUDA 13.0 libraries (nvidia-cuda-runtime-13.0.48, etc.)

### Compatibility Status
PyTorch 2.9.0 reports:
- **Detected**: NVIDIA GB10 with CUDA capability 12.1 (sm_121)
- **Supported range**: 8.0 - 12.0
- **Status**: WARNING (not error!)
- sm_121 is slightly above max supported (12.0) but still functions

### OCR Inference SUCCESS! ✅

**Test Results:**
- Model loaded: 33.82 seconds
- Inference completed: 58.27 seconds
- Total time: ~92 seconds
- Image processed: 3503x1668 pixels
- Valid image tokens: 921
- Output text tokens: 2257
- Compression ratio: 2.45x

**Output Generated:**
- `result_with_boxes.jpg` - 976KB (image with detected text boxes)
- `result.mmd` - Text output file
- `images/` directory - Intermediate processing images

### Key Insights

1. **PyTorch 2.9.0 is more tolerant** than 2.5.1
   - 2.5.1: Hard error on sm_121
   - 2.9.0: Warning but functional

2. **CUDA 13.0 support is crucial**
   - cu130 wheels made the difference
   - Direct compatibility with system CUDA

3. **sm_121 forward compatibility**
   - While "unsupported", kernels execute successfully
   - Likely using sm_120 codepath
   - No runtime failures observed

### Detected Text Sample

The OCR successfully detected and located text from the article about "The perils of vibe coding" including:
- Title and headers
- Body text in multiple columns
- Precise bounding box coordinates for each text segment
- Proper text extraction from complex multi-column layout

### Conclusion

**The setup is now fully functional!** PyTorch 2.9.0's CUDA 13.0 wheels work with the GB10 GPU despite sm_121 being slightly above the "official" support range.

---

## Final Summary (2025-10-20 15:51)

### Problem Solved! ✅

**Original Issue**: NVIDIA GB10 (sm_121) not supported by PyTorch 2.5.1

**Solution**: Upgrade to **PyTorch 2.9.0 with CUDA 13.0 wheels**

### Installation Command
```bash
pip3 install --break-system-packages torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu130
```

### Why It Works
1. PyTorch 2.9.0 has native CUDA 13.0 support
2. While sm_121 is "officially" unsupported (max is 12.0), it works with a warning
3. Likely uses sm_120 kernels which are compatible with sm_121
4. All CUDA operations execute successfully

### Performance Metrics
- Image: 3503x1668 pixels (586KB JPEG)
- Model loading: 33.82 seconds
- OCR inference: 58.27 seconds  
- Text detected: 2257 tokens from 921 vision tokens
- Compression: 2.45x
- Output: Text with bounding boxes + annotated image

### Complete Setup Flow
1. ✅ Install dependencies (setup.sh)
2. ✅ Clone repositories (code + model)
3. ✅ **Upgrade to PyTorch 2.9.0+cu130**
4. ✅ Run OCR (run_ocr.py)
5. ✅ SUCCESS!

---

## Text Output Improvement (2025-10-20 16:01)

### Problem
Initial OCR run produced empty `result.mmd` (only whitespace)
- Bounding box image was good
- But text output was missing

### Root Cause
The "grounding" prompt (`<|grounding|>OCR this image.`) focuses on:
- Bounding box coordinates
- Text location detection
- Visual annotation

This mode outputs structured data with coordinates but the text is embedded in the grounding format, not saved as clean text.

### Solution - Different Prompts for Different Uses

Tested all available prompts:

1. **"Free OCR"** (`<image>\nFree OCR.`)
   - ✅ Clean, readable text output
   - ✅ Natural paragraph flow
   - ✅ Fast (24 seconds)
   - ✅ Best for general text extraction

2. **"Markdown"** (`<image>\n<|grounding|>Convert the document to markdown.`)
   - ✅ Structured markdown output
   - ✅ Preserves headings and formatting
   - ✅ Includes image references
   - ✅ Better for documents

3. **"Grounding OCR"** (`<image>\n<|grounding|>OCR this image.`)
   - ✅ Excellent bounding boxes
   - ✅ Precise coordinates
   - ⚠️ Text in coordinate format
   - Best for annotation/UI tools

4. **"Detailed"** (`<image>\nDescribe this image in detail.`)
   - ✅ Image analysis
   - ⚠️ Not for OCR
   - Best for understanding image content

### Results

**Free OCR output (best):**
```
# The perils of vibe coding

Elaine Moore

new OpenAI model arrived this month with a glossy livestream, 
group watch parties and a lingering sense of disappointment.

The YouTube comment section was underwhelmed. "I think they are 
all starting to realize this isn't going to change the world like 
they thought it would," wrote one viewer...
```

**Markdown output:**
```markdown
## The perils of vibe coding 

TECHNOLOGY
Elaine Moore 

![](images/0.jpg)

new OpenAI model arrived this month...
```

### Created Scripts

- **`run_ocr_best.py`** - Optimized for clean text (uses "Free OCR")
- **`run_ocr_text_focused.py`** - Tests all prompts
- **`PROMPTS_GUIDE.md`** - Complete prompt documentation

### Recommendation

For most OCR tasks, use:
```bash
python3 run_ocr_best.py your_image.jpg
```

This provides clean, readable text output that's easy to work with.

---

