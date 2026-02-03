# Running GLM-OCR in the Browser with Transformers.js

## Executive Summary

**Short answer: Not currently possible.** There is a critical blocker preventing GLM models from being exported to ONNX format, which is required for Transformers.js.

This investigation explores what would be needed to run [zai-org/GLM-OCR](https://huggingface.co/zai-org/GLM-OCR) in the browser using Transformers.js, identifying the current blockers and potential alternatives.

## GLM-OCR Model Overview

GLM-OCR is a 0.9B parameter multimodal OCR model built on the GLM-V encoder-decoder architecture. It consists of:

| Component | Description |
|-----------|-------------|
| **Visual Encoder** | CogViT - pre-trained on large-scale image-text data |
| **Cross-Modal Connector** | Lightweight module with efficient token downsampling |
| **Language Decoder** | GLM-0.5B for text generation |
| **Layout Analysis** | PP-DocLayout-V3 (two-stage pipeline) |

The model achieves state-of-the-art performance (94.62 on OmniDocBench V1.5) and supports document parsing, formula recognition, table recognition, and information extraction.

## What Would Be Required

### 1. ONNX Conversion (BLOCKED)

Transformers.js requires models to be in ONNX format. GLM models **cannot currently be exported to ONNX** due to a PyTorch bug.

**The Issue:**
- GitHub Issue: [huggingface/transformers#35021](https://github.com/huggingface/transformers/issues/35021)
- Failed Fix Attempt: [huggingface/transformers#35018](https://github.com/huggingface/transformers/pull/35018)

**Root Cause:**
```
AssertionError in torch.onnx.symbolic_opset9.cat
```

The `repeat_interleave` operation in GLM's attention mechanism causes failures during ONNX graph compilation. This is traced to a PyTorch bug ([pytorch/pytorch#145100](https://github.com/pytorch/pytorch/issues/145100)), not a transformers-specific issue.

The Transformers.js maintainer (xenova) attempted a fix but was unsuccessful. Similar models like Gemma export correctly, suggesting this is GLM-architecture-specific.

**Potential Workaround:** Override the `repeat_interleave` operator in HuggingFace Optimum, but this hasn't been implemented.

### 2. Architecture Support in Transformers.js

Even if ONNX export was fixed, Transformers.js would need:

- **GLM-V architecture support** in the conversion script
- **Multimodal model handling** (vision encoder + language decoder split)
- **Custom operators** for GLM-specific attention patterns

Currently supported multimodal models in Transformers.js include LLaVA, PaliGemma, CLIP, and SigLIP - but not GLM-V.

### 3. PP-DocLayout-V3 Port

GLM-OCR uses PP-DocLayout-V3 from PaddlePaddle for layout analysis. This would require:

- Separate ONNX conversion from PaddlePaddle format
- Custom JavaScript implementation for layout parsing
- Or implementing layout detection with an existing Transformers.js model

### 4. Model Size Optimization

At 0.9B parameters, the model is large for browser deployment:

| Precision | Size | Browser Feasibility |
|-----------|------|---------------------|
| FP32 | ~3.6 GB | Not feasible |
| FP16 | ~1.8 GB | Very difficult |
| INT8 (q8) | ~900 MB | Borderline |
| INT4 (q4) | ~450 MB | Possible with WebGPU |

For comparison, typical Transformers.js models are 50-200MB. PaliGemma-3B at ~1.5GB represents the upper limit of what's practical.

### 5. WebGPU Acceleration

Browser inference of a 0.9B model would require:
- WebGPU support (experimental in most browsers)
- Modern GPU with sufficient VRAM
- Optimized ONNX operators

## Theoretical Conversion Process

If the blockers were resolved, conversion would follow this process:

```bash
# 1. Install dependencies
pip install transformers optimum onnx onnxruntime

# 2. Convert model to ONNX (currently fails)
optimum-cli export onnx \
  --model zai-org/GLM-OCR \
  --task image-text-to-text \
  ./glm-ocr-onnx/

# 3. Quantize for smaller size
optimum-cli onnxruntime quantize \
  --avx512 \
  --onnx_model ./glm-ocr-onnx/ \
  --output ./glm-ocr-onnx-q4/

# 4. Use in browser
```

```javascript
import { pipeline } from '@huggingface/transformers';

const ocr = await pipeline('image-to-text', 'path/to/glm-ocr-onnx', {
  device: 'webgpu',
  dtype: 'q4',
});

const result = await ocr(imageData);
```

## Alternatives That Work Today

### TrOCR (Recommended for Browser)

Microsoft's TrOCR is available in Transformers.js:

```javascript
import { pipeline } from '@huggingface/transformers';

const ocr = await pipeline('image-to-text', 'Xenova/trocr-small-printed');
const result = await ocr(imageUrl);
```

- **Size**: ~60MB (encoder + decoder)
- **Pros**: Works today, good for printed text
- **Cons**: Less capable than GLM-OCR, no layout analysis

### Other Browser OCR Options

| Model | Size | Features |
|-------|------|----------|
| Tesseract.js | ~10MB | Classic OCR, 100+ languages |
| TrOCR-small | ~60MB | Transformer-based, printed text |
| Donut | ~200MB | Document understanding |

### Server-Side GLM-OCR

For full GLM-OCR capabilities, server deployment works today:

```python
# Using vLLM
vllm serve zai-org/GLM-OCR --port 8080

# Using SGLang
python -m sglang.launch_server --model zai-org/GLM-OCR --port 8080

# Using Ollama
ollama run glm-ocr
```

Then call from browser via API.

## Timeline Estimate

For GLM-OCR browser support to become possible:

1. **PyTorch ONNX bug fix** - Unknown timeline, depends on PyTorch team
2. **Optimum workaround** - Could be weeks to months if prioritized
3. **Transformers.js GLM support** - Additional work after ONNX export works
4. **PP-DocLayout port** - Separate effort, possibly weeks

Realistically, this is **6+ months away** assuming active development focus.

## Conclusion

Running GLM-OCR in the browser with Transformers.js is **not currently feasible** due to:

1. **Critical blocker**: GLM models cannot be exported to ONNX (PyTorch bug)
2. **Architecture gap**: GLM-V not yet supported in Transformers.js
3. **Dependency gap**: PP-DocLayout-V3 has no JavaScript port
4. **Size challenge**: 0.9B parameters is at the upper limit for browsers

**Recommendations:**

- **For browser OCR today**: Use TrOCR or Tesseract.js
- **For GLM-OCR quality**: Deploy server-side and call via API
- **For future browser use**: Monitor [transformers#35021](https://github.com/huggingface/transformers/issues/35021) for ONNX export fix

## Sources

- [GLM-OCR on HuggingFace](https://huggingface.co/zai-org/GLM-OCR)
- [Transformers.js Documentation](https://huggingface.co/docs/transformers.js/index)
- [GLM ONNX Export Issue](https://github.com/huggingface/transformers/issues/35021)
- [GLM ONNX Fix Attempt (Closed)](https://github.com/huggingface/transformers/pull/35018)
- [GLM Model Documentation](https://huggingface.co/docs/transformers/model_doc/glm)
- [GLM-4V Documentation](https://huggingface.co/docs/transformers/v5.0.0/en/model_doc/glm4v)
- [Transformers.js GitHub](https://github.com/huggingface/transformers.js)
