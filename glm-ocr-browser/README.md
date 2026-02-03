# Running GLM-OCR in the Browser with Transformers.js

## Executive Summary

**Short answer: Not currently possible.** GLM-OCR is a brand-new model (released ~11 hours ago) that isn't yet supported by the ONNX export toolchain needed for Transformers.js.

This investigation explores what would be needed to run [zai-org/GLM-OCR](https://huggingface.co/zai-org/GLM-OCR) in the browser using Transformers.js, identifying current blockers and alternatives.

## GLM-OCR Model Overview

GLM-OCR is a 0.9B parameter multimodal OCR model using the GLM-V encoder-decoder architecture:

| Component | Description |
|-----------|-------------|
| **Visual Encoder** | CogViT - 24 layers, 1024 hidden size, 336×336 input |
| **Cross-Modal Connector** | Token downsampling (spatial_merge_size=2) |
| **Language Decoder** | GLM-0.5B - 16 layers, 1536 hidden size |
| **Layout Analysis** | PP-DocLayout-V3 (external, PaddlePaddle) |

The model achieves state-of-the-art performance (94.62 on OmniDocBench V1.5).

## Critical Blockers

### 1. Model Is Too New (PRIMARY BLOCKER)

GLM-OCR support was [merged into transformers](https://github.com/huggingface/transformers/pull/43391) just ~1 week ago and requires `transformers >= 5.0.1dev0`:

```python
# In transformers 5.0.0 (latest stable):
'glm_ocr' in CONFIG_MAPPING  # False

# In transformers 5.0.1dev0 (git HEAD):
'glm_ocr' in CONFIG_MAPPING  # True
```

**Dependency Conflict:**
```
optimum[onnxruntime] requires transformers < 4.58.0
GLM-OCR requires transformers >= 5.0.1dev0
```

When you install optimum (needed for ONNX export), it downgrades transformers and GLM-OCR becomes unrecognized.

### 2. No ONNX Export Config

Even with compatible versions, `glm_ocr` has no ONNX export configuration in Optimum:

```python
from optimum.exporters.tasks import TasksManager
TasksManager.get_supported_tasks_for_model_type('glm_ocr', 'onnx')
# KeyError: 'glm_ocr is not supported yet'
```

Note: Most vision-language models (LLaVA, PaliGemma, Qwen2-VL) also lack standard Optimum export - they require custom conversion scripts.

### 3. No Transformers.js Implementation

Examined [transformers.js source](https://github.com/huggingface/transformers.js):

- **Exists**: `GlmModel`, `GlmForCausalLM` (text-only GLM)
- **Missing**: `GlmOcrForConditionalGeneration`, `glm_ocr` model type

Would need to add:
- Vision encoder class (similar to `Glm4vVisionModel`)
- Conditional generation class with `_merge_input_ids_with_image_features`
- Image processor integration

### 4. Potential PyTorch ONNX Issues

There's a known [ONNX export bug](https://github.com/huggingface/transformers/issues/35021) affecting `GlmForCausalLM` (text-only):

```
AssertionError in torch.onnx.symbolic_opset9.cat
```

**Important:** This bug is for `GlmForCausalLM`, not `GlmOcrForConditionalGeneration`. We cannot verify if it affects GLM-OCR because of the dependency conflict blocking any export attempt.

### 5. Model Size

| Precision | Size | Browser Feasibility |
|-----------|------|---------------------|
| FP32 | ~3.6 GB | Not feasible |
| FP16 | ~1.8 GB | Very difficult |
| INT8 (q8) | ~900 MB | Borderline |
| INT4 (q4) | ~450 MB | Possible with WebGPU |

For reference, [onnx-community/paligemma2-3b-pt-224](https://huggingface.co/onnx-community/paligemma2-3b-pt-224) shows the ONNX structure needed:
- `vision_encoder.onnx` (~236-831 MB)
- `embed_tokens.onnx` (~593 MB - 1.19 GB)
- `decoder_model_merged.onnx` (~1.5-10.5 GB)

## What Would Be Required

To run GLM-OCR in browser:

| Requirement | Status | Effort |
|-------------|--------|--------|
| Transformers.js framework | ✅ Available | - |
| transformers/optimum compatibility | ❌ Blocked | Wait for optimum update |
| `glm_ocr` ONNX config in Optimum | ❌ Missing | PR needed |
| Custom ONNX export script | ❌ Missing | Days of work |
| `GlmOcrForConditionalGeneration` in TJS | ❌ Missing | Days of work |
| PyTorch ONNX fix (if needed) | ⚠️ Unknown | Depends on PyTorch |
| PP-DocLayout-V3 port | ❌ Missing | Weeks of work |
| WebGPU for 450MB+ model | ⚠️ Experimental | - |

## Theoretical Conversion Process

If all blockers were resolved:

```bash
# 1. Install compatible versions (not currently possible)
pip install 'transformers>=5.0.1' 'optimum>=X.X.X'

# 2. Convert to ONNX (would need custom config)
python convert_glm_ocr.py \
  --model zai-org/GLM-OCR \
  --output ./glm-ocr-onnx/

# 3. Quantize
optimum-cli onnxruntime quantize \
  --onnx_model ./glm-ocr-onnx/ \
  --output ./glm-ocr-onnx-q4/
```

```javascript
// Browser usage (hypothetical)
import { pipeline } from '@huggingface/transformers';

const ocr = await pipeline('image-to-text', 'path/to/glm-ocr-onnx', {
  device: 'webgpu',
  dtype: 'q4',
});

const result = await ocr(imageData);
```

## Alternatives That Work Today

### TrOCR (Recommended for Browser)

```javascript
import { pipeline } from '@huggingface/transformers';

const ocr = await pipeline('image-to-text', 'Xenova/trocr-small-printed');
const result = await ocr(imageUrl);
```

- **Size**: ~60MB
- **Pros**: Works today, good for printed text
- **Cons**: Less capable than GLM-OCR

### Other Options

| Model | Size | Features |
|-------|------|----------|
| Tesseract.js | ~10MB | Classic OCR, 100+ languages |
| TrOCR-small | ~60MB | Transformer-based, printed text |
| Donut | ~200MB | Document understanding |
| [FastVLM-0.5B](https://huggingface.co/onnx-community/FastVLM-0.5B-ONNX) | ~500MB | VLM with browser demo |

### Server-Side GLM-OCR

For full GLM-OCR capabilities today:

```bash
# vLLM
vllm serve zai-org/GLM-OCR --port 8080

# SGLang
python -m sglang.launch_server --model zai-org/GLM-OCR --port 8080

# Ollama
ollama run glm-ocr
```

Then call from browser via API.

## Timeline

GLM-OCR browser support requires:

1. **Optimum compatibility** with transformers 5.x - Weeks to months
2. **ONNX export config** for glm_ocr - Community PR needed
3. **Transformers.js implementation** - Days of development
4. **Testing/debugging** - Unknown

Realistically: **3-6+ months** assuming someone prioritizes this work.

## Conclusion

Running GLM-OCR in the browser with Transformers.js is **not currently feasible** due to:

1. **Primary blocker**: Model requires transformers 5.0.1+ but ONNX tools require <4.58
2. **Missing support**: No ONNX config, no Transformers.js implementation
3. **Untested**: Cannot verify if PyTorch ONNX bugs affect this model
4. **Size**: 450MB+ at q4 quantization requires WebGPU

**Recommendations:**

- **For browser OCR today**: Use TrOCR, Tesseract.js, or FastVLM
- **For GLM-OCR quality**: Deploy server-side and call via API
- **To help**: Contribute ONNX export config to Optimum, or Transformers.js implementation

## Sources

- [GLM-OCR on HuggingFace](https://huggingface.co/zai-org/GLM-OCR)
- [GLM-OCR Support PR #43391](https://github.com/huggingface/transformers/pull/43391)
- [Transformers.js GitHub](https://github.com/huggingface/transformers.js)
- [GLM ONNX Export Issue #35021](https://github.com/huggingface/transformers/issues/35021) (text-only GLM)
- [PaliGemma2 ONNX Example](https://huggingface.co/onnx-community/paligemma2-3b-pt-224)
- [FastVLM Browser Demo](https://huggingface.co/onnx-community/FastVLM-0.5B-ONNX)
