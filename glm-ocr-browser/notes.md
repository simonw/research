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

### Summary of Requirements

To run GLM-OCR in browser, you would need:

1. ✅ Transformers.js framework (available)
2. ❌ GLM ONNX export fix (blocked by PyTorch bug)
3. ❌ GLM-V architecture support in Transformers.js conversion script
4. ❌ PP-DocLayout-V3 ONNX port
5. ⚠️ Model size reduction through quantization
6. ⚠️ WebGPU for acceptable performance

