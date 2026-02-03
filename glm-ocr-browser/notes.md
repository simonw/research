# GLM-OCR in Browser Investigation

## Initial Research

### GLM-OCR Model Details (from HuggingFace)

- **Architecture**: GLM-V encoder-decoder (multimodal OCR)
- **Parameters**: 0.9B (900 million)
- **Components**:
  - Visual Encoder: CogViT (pre-trained on large-scale image-text data)
  - Cross-Modal Connector: Lightweight with token downsampling
  - Language Decoder: GLM-0.5B
  - Layout Analysis: PP-DocLayout-V3 (two-stage pipeline)
- **License**: MIT (model), Apache 2.0 (PP-DocLayout-V3)
- **Transformers API**: Uses `AutoModelForImageTextToText`

### Key Questions to Answer

1. Does Transformers.js support the GLM-V architecture?
2. What ONNX conversion would be needed?
3. Is the model size (0.9B params) feasible for browser?
4. What are the dependencies and preprocessing requirements?

---

## Research Progress

### Transformers.js Overview

Transformers.js runs HuggingFace models in the browser using ONNX Runtime. Key findings:

- **ONNX Runtime**: All models must be converted to ONNX format
- **Quantization**: Supports fp32, fp16, q8, q4 for smaller model sizes
- **WebGPU**: Experimental GPU acceleration available
- **Model Size**: Typical browser models are 50-500MB; 0.9B params would be ~1.8GB in fp32, ~450MB in q4
- **Multimodal Support**: Limited - mainly CLIP, SigLIP, LLaVA, PaliGemma

### GLM ONNX Export - CRITICAL BLOCKER

Found a **critical blocker**: GLM models cannot currently be exported to ONNX.

**Issue**: [huggingface/transformers#35021](https://github.com/huggingface/transformers/issues/35021)
**Attempted Fix (Closed WIP)**: [huggingface/transformers#35018](https://github.com/huggingface/transformers/pull/35018)

**Root Cause**:
- AssertionError in PyTorch's ONNX conversion during `torch.onnx.symbolic_opset9.cat`
- The `repeat_interleave` operation in GLM's attention mechanism causes issues
- This is a PyTorch bug (pytorch/pytorch#145100), not a transformers issue
- Similar models like Gemma export successfully, but GLM-specific attention patterns fail

**Workaround Suggested**: Override `repeat_interleave` operator in Optimum, but not implemented.

### GLM-4V / GLM-V Architecture Details

GLM-4V uses `Glm4vForConditionalGeneration` with:
- `Glm4vVisionModel`: Vision encoder (patch_size=14, hidden_size=1536, 24 layers)
- `Glm4vTextModel`: Language decoder (GLM-based, 40 layers, 4096 hidden)
- Multimodal projector connecting vision to language

GLM-OCR specifically uses:
- CogViT (likely related to Glm4vVisionModel)
- GLM-0.5B decoder (smaller than GLM-4V's 9B)

### Model Size Analysis

GLM-OCR at 0.9B parameters:
- **FP32**: ~3.6 GB (infeasible for browser)
- **FP16**: ~1.8 GB (very large for browser)
- **INT8 (q8)**: ~900 MB (borderline feasible)
- **INT4 (q4)**: ~450 MB (potentially feasible but still large)

For comparison, typical Transformers.js models:
- DistilBERT: ~67MB (q8)
- Whisper-tiny: ~39MB
- PaliGemma-3B: ~1.5GB (very large, needs WebGPU)

### PP-DocLayout-V3 (Layout Analysis)

GLM-OCR uses a two-stage pipeline:
1. PP-DocLayout-V3 for document layout analysis
2. Parallel OCR recognition across detected regions

PP-DocLayout-V3 is from PaddlePaddle, not HuggingFace:
- Separate model/framework
- Would need its own ONNX conversion
- No known Transformers.js port exists

### Alternative Browser OCR Models

Models that **do work** in Transformers.js:

1. **TrOCR** - Microsoft's OCR model
   - Already has ONNX support
   - Available in Transformers.js: `Xenova/trocr-small-printed`
   - ~60MB per model (encoder + decoder)

2. **Donut** - Document understanding transformer
   - Encoder-decoder for document parsing
   - Some ONNX support exists

3. **LayoutLM** variants - For document understanding
   - Various sizes available

---

## Hands-On Testing (2026-02-03)

### GLM-OCR is Brand New

The model was released just 11 hours ago! Key finding:

- **PR #43391**: [GLM-OCR Support](https://github.com/huggingface/transformers/pull/43391) merged ~1 week ago
- **Requires**: `transformers >= 5.0.1dev0` (git HEAD only)
- **Not in**: transformers 5.0.0 (latest stable release)

### Transformers.js Code Analysis

Cloned transformers.js repo and examined:

1. **GLM text-only support exists** in `src/models.js`:
   - `GlmModel`, `GlmForCausalLM` (line ~4648)
   - Model type `'glm'` in configs

2. **GLM-OCR NOT supported**:
   - No `GlmOcrForConditionalGeneration`
   - No `glm_ocr` model type
   - No vision encoder handling for GLM-OCR

3. **Similar VLM patterns exist** (LLaVA, PaliGemma, Qwen2-VL):
   - Vision encoder + decoder split
   - `_merge_input_ids_with_image_features` method
   - Would need similar implementation for GLM-OCR

### ONNX Conversion Structure (from onnx-community/paligemma2)

For VLMs, Transformers.js needs three ONNX files:
1. `vision_encoder.onnx` - Image processing
2. `embed_tokens.onnx` - Text embeddings
3. `decoder_model_merged.onnx` - Language model

### Attempted ONNX Export - FAILED

**Blocker: Dependency Conflict**

```
optimum[onnxruntime] requires transformers < 4.58.0
GLM-OCR requires transformers >= 5.0.1dev0
```

When installing optimum, it downgrades transformers to 4.57.6, which doesn't have `glm_ocr` support.

Error message:
```
ValueError: The checkpoint you are trying to load has model type `glm_ocr`
but Transformers does not recognize this architecture.
```

### Optimum ONNX Export Support Check

Ran TasksManager checks - neither GLM-OCR nor multimodal VLMs are supported:

```python
TasksManager.get_supported_tasks_for_model_type('glm_ocr', 'onnx')
# KeyError: 'glm_ocr is not supported yet'

# Also not supported: llava, paligemma, qwen2_vl, florence2, idefics3
```

This means **even working VLMs in Transformers.js don't use standard Optimum export** - they likely have custom conversion scripts or configs.

### GLM-OCR Config Details

```json
{
  "architectures": ["GlmOcrForConditionalGeneration"],
  "model_type": "glm_ocr",
  "text_config": {
    "model_type": "glm_ocr_text",
    "hidden_size": 1536,
    "num_hidden_layers": 16,
    "num_attention_heads": 16,
    "num_key_value_heads": 8,
    "vocab_size": 59392,
    "max_position_embeddings": 131072
  },
  "vision_config": {
    "model_type": "glm_ocr_vision",
    "hidden_size": 1024,
    "depth": 24,
    "num_heads": 16,
    "image_size": 336,
    "patch_size": 14
  }
}
```

### ONNX Export Issues (#35021 vs GLM-OCR)

Important clarification:
- **Issue #35021** is about `GlmForCausalLM` (text-only GLM)
- **GLM-OCR** uses `GlmOcrForConditionalGeneration` (different architecture)
- The PyTorch `repeat_interleave` bug **may or may not apply** to GLM-OCR
- Can't test because of the dependency conflict

---

## Summary of Requirements (Updated)

To run GLM-OCR in browser, you would need:

1. ✅ Transformers.js framework (available)
2. ❌ **NEW**: Dependency compatibility (transformers 5.0.1+ with optimum)
3. ❌ `glm_ocr` ONNX config in Optimum
4. ❌ `GlmOcrForConditionalGeneration` class in Transformers.js
5. ❌ Custom ONNX export script for vision/text split
6. ⚠️ May need PyTorch ONNX fixes if GLM attention patterns fail
7. ❌ PP-DocLayout-V3 port (if layout analysis needed)
8. ⚠️ Model size reduction (~450MB at q4)
9. ⚠️ WebGPU for performance

