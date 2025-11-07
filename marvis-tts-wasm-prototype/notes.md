# Marvis TTS WebAssembly Prototype - Research Notes

## Project Goal
Run Marvis-AI/marvis-tts-100m-v0.2 TTS model in a web browser using WebAssembly

## Investigation Log

### Initial Research - 2025-11-07

**Target Model**: https://huggingface.co/Marvis-AI/marvis-tts-100m-v0.2

**Attempt 1**: Direct HuggingFace page fetch
- Result: 403 error (access restricted or rate limited)
- Next: Web search for model information

**Findings from GitHub and Web Search**:

Available Models:
- Main model: `Marvis-AI/marvis-tts-250m-v0.1` (250M params backbone + 60M params decoder)
- Note: The specific `marvis-tts-100m-v0.2` model was not found in search results
- Likely will work with the 250m-v0.1 model instead

Architecture:
- Dual-transformer design based on Sesame CSM-1B
- Multimodal Backbone (250M): Processes text and audio sequences for zeroth codebook
- Audio Decoder (60M): Handles remaining 31 codebook levels
- Uses Residual Vector Quantization (RVQ) tokens with Kyutai's Mimi codec
- Output: 24kHz WAV audio

Dependencies:
- MLX framework (`mlx-audio`) - optimized for Apple Silicon
- HuggingFace Transformers (AutoProcessor + CsmForConditionalGeneration)
- Requires voice cloning: 10-second reference audio

File Format:
- PyTorch/Transformers format (no ONNX version mentioned)
- Model size: ~500MB quantized

### WebAssembly Runtime Options Investigation

**Research into Transformers.js**:
- Supports TTS through ONNX Runtime Web + WebAssembly
- Currently supports SpeechT5 model (Microsoft's TTS model)
- Uses ONNX format, WebAssembly for acceleration, Web Workers for parallel processing
- Available since version 2.7 with TTS support

**CSM (Conversational Speech Model) Support**:
- CSM is available in HuggingFace Transformers Python library (v4.52.1+)
- NOT yet available in Transformers.js for browser use
- No ONNX export found for Marvis TTS model
- Marvis TTS is optimized for MLX (Apple Silicon) and PyTorch Transformers

**Key Findings**:
1. Marvis TTS (CSM-based) is NOT browser-ready currently
2. No ONNX export exists for Marvis TTS yet
3. Would require custom ONNX conversion from PyTorch model
4. Transformers.js DOES support TTS via SpeechT5 model

### Decision: Dual Approach

**Approach 1**: Build working prototype with SpeechT5 (currently available)
- Demonstrates browser-based TTS with WebAssembly
- Shows what's possible today
- Uses Transformers.js with ONNX Runtime Web

**Approach 2**: Document Marvis TTS browser port requirements
- Technical analysis of porting challenges
- Steps needed for ONNX conversion
- Architecture considerations for browser deployment

### Building the Prototype

**Prototype Stack**:
- Transformers.js library (@huggingface/transformers)
- Xenova/speecht5_tts model (Microsoft SpeechT5 with ONNX weights)
- Speaker embeddings for voice customization
- Pure HTML/JavaScript (no build tools needed for basic version)

**Implementation Plan**:
1. Create HTML page with text input and playback controls
2. Load Transformers.js from CDN or npm
3. Initialize text-to-speech pipeline with SpeechT5
4. Load speaker embeddings
5. Generate audio from text input
6. Play audio in browser using Web Audio API

### Prototype Implementation - 2025-11-07

**Created: tts-demo.html**

Features implemented:
- Clean, modern UI with status indicators
- Lazy loading of TTS model (loaded on first use)
- Text input with textarea
- Voice selection dropdown (placeholder for multiple embeddings)
- Real-time status updates during synthesis
- Progress indicator for long operations
- Audio playback using Web Audio API
- Downloadable WAV file generation
- Error handling and user feedback

Technical stack:
- Transformers.js v2.17.2 from CDN
- Xenova/speecht5_tts model (ONNX format)
- Speaker embeddings from HuggingFace datasets
- Web Audio API for playback
- Custom WAV file generation (no external dependencies)

Key code sections:
1. Model initialization with caching
2. Text-to-speech synthesis pipeline
3. Float32Array to AudioBuffer conversion
4. WAV file generation for download
5. Audio playback controls

## Porting Marvis TTS to Browser/WebAssembly

### Current State Analysis

**Marvis TTS Architecture**:
- Based on Sesame CSM-1B (Conversational Speech Model)
- Dual-transformer design:
  - Multimodal Backbone: 250M parameters
  - Audio Decoder: 60M parameters
- Uses Mimi codec for RVQ (Residual Vector Quantization)
- Total model size: ~500MB quantized
- Output: 24kHz WAV audio

**Available Formats**:
- PyTorch/Transformers (primary)
- MLX (Apple Silicon optimized)
- No ONNX export currently available

### Technical Challenges for Browser Deployment

1. **Model Format Conversion**
   - Need to export PyTorch model to ONNX format
   - CSM architecture must be ONNX-compatible
   - All custom operations must be supported in ONNX Runtime
   - Challenge: Mimi codec integration with ONNX

2. **Model Size Optimization**
   - 500MB is manageable for browser download but large
   - Would benefit from quantization (INT8 or INT4)
   - Could explore model splitting/streaming for progressive loading
   - Browser cache can help with subsequent loads

3. **Dependency Resolution**
   - Mimi codec: Needs JavaScript/WASM implementation
   - RVQ operations: Must be ONNX-compatible or reimplemented
   - Audio processing: Can use Web Audio API
   - Custom ops: May need WASM modules

4. **Runtime Environment**
   - ONNX Runtime Web supports most operations
   - WebAssembly provides near-native performance
   - SIMD support varies by browser
   - Memory constraints in browser environment

### Required Steps for Browser Port

**Phase 1: Model Export**
1. Export Marvis TTS from PyTorch to ONNX format
   - Use torch.onnx.export() with custom op registration
   - Handle CSM-specific operations
   - Test ONNX model matches PyTorch output

2. Validate ONNX model
   - Verify with ONNX Runtime Python
   - Check all operations are supported
   - Measure inference performance

**Phase 2: Codec Implementation**
1. Port Mimi codec to JavaScript/WASM
   - Identify codec operations
   - Implement in JavaScript or compile to WASM
   - Create WASM bindings if needed

2. Integrate with ONNX model
   - Pre/post-processing pipeline
   - Coordinate data flow between codec and model

**Phase 3: Browser Integration**
1. Create JavaScript wrapper using Transformers.js architecture
   - Model loading and caching
   - Speaker embeddings handling
   - Streaming synthesis support

2. Optimize for browser
   - Implement model quantization
   - Add Web Worker support for non-blocking UI
   - Implement progressive loading
   - Add error handling and fallbacks

**Phase 4: Testing and Optimization**
1. Cross-browser testing (Chrome, Firefox, Safari, Edge)
2. Performance benchmarking
3. Memory usage optimization
4. Network loading optimization

### Estimated Complexity

**Timeline**: 4-8 weeks for experienced ML engineer
- ONNX export: 1-2 weeks
- Codec porting: 2-3 weeks
- Browser integration: 1-2 weeks
- Testing/optimization: 1 week

**Technical Difficulty**: High
- Requires expertise in PyTorch, ONNX, JavaScript, WebAssembly
- Custom codec implementation is non-trivial
- CSM architecture may have unsupported operations

**Blockers**:
1. CSM model must be ONNX-exportable
2. Mimi codec operations must be implementable in WASM
3. Model size must fit browser memory constraints
4. Performance must be acceptable (target: <5s for short phrases)

### Alternative Approaches

**Option 1: Server-side API**
- Keep Marvis TTS on server
- Expose REST API for browser clients
- Streaming audio response
- Pros: No porting needed, full performance
- Cons: Requires server, network latency, privacy concerns

**Option 2: WebGPU Acceleration**
- Use WebGPU instead of WASM for compute
- Could enable better performance for transformer models
- Pros: Faster inference, native GPU support
- Cons: Limited browser support (Chrome 113+), experimental

**Option 3: Hybrid Approach**
- Lighter-weight model for browser (e.g., SpeechT5)
- Marvis TTS on server for high-quality synthesis
- Progressive enhancement based on capabilities
- Pros: Works everywhere, optimal experience when possible
- Cons: Complexity in maintaining two systems

### Recommendation

For production use: Start with SpeechT5 (demonstrated in prototype) for broad compatibility

For research/advanced use: Pursue ONNX export of Marvis TTS with estimated 6-8 week timeline

For immediate deployment: Consider server-side Marvis TTS API with browser client

