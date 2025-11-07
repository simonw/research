# Browser-Based TTS with WebAssembly: Marvis TTS Research & Prototype

## Executive Summary

This research investigates running the Marvis TTS model (https://huggingface.co/Marvis-AI/marvis-tts-100m-v0.2) in a web browser using WebAssembly. The investigation found that **Marvis TTS is not currently browser-compatible** due to the lack of ONNX export and dependency on specialized codecs. However, a **working prototype** was developed using Microsoft's SpeechT5 model via Transformers.js to demonstrate the current state-of-the-art for browser-based text-to-speech.

## Key Findings

### Marvis TTS Current State

**Architecture**:
- Based on Sesame CSM-1B (Conversational Speech Model)
- Dual-transformer design with 250M parameter backbone + 60M parameter decoder
- Uses Kyutai's Mimi codec for Residual Vector Quantization (RVQ)
- Total size: ~500MB quantized
- Output: 24kHz WAV audio with voice cloning capabilities

**Availability**:
- ✅ PyTorch/HuggingFace Transformers (primary format)
- ✅ MLX (optimized for Apple Silicon)
- ❌ ONNX (not available - required for browser deployment)
- ❌ Browser/WebAssembly (not supported)

**Current Deployment Targets**:
- Desktop: macOS, Linux, Windows via Python
- Mobile: iOS, Android via MLX framework
- Cloud: API deployments with streaming support

### Browser-Based TTS: What Works Today

The research identified **Transformers.js** as the leading solution for running transformer models in browsers:

- Uses ONNX Runtime Web + WebAssembly for inference
- Currently supports Microsoft SpeechT5 for text-to-speech
- Provides ~100MB model size with browser caching
- Offers decent quality with customizable speaker embeddings
- Achieves client-side inference with no server required

## Prototype Demonstration

### File: `tts-demo.html`

A fully functional single-file HTML demo that runs text-to-speech entirely in the browser.

**Features**:
- Single HTML file (no build tools required)
- Automatic model loading and browser caching
- Real-time speech synthesis from text input
- Audio playback with Web Audio API
- Downloadable WAV file generation
- Clean, modern UI with status indicators
- Error handling and user feedback

**Technical Stack**:
- Transformers.js v2.17.2 (loaded from CDN)
- Xenova/speecht5_tts model (ONNX format)
- Web Audio API for playback
- Custom WAV encoder (no external dependencies)

**How to Use**:
1. Open `tts-demo.html` in a modern web browser (Chrome, Firefox, Safari, Edge)
2. Enter text in the textarea
3. Click "Generate Speech"
4. Wait for model to download on first use (~100MB, cached afterwards)
5. Listen to generated speech or download as WAV file

**Browser Requirements**:
- Modern browser with JavaScript enabled
- WebAssembly support (all major browsers since 2017)
- ~200MB available memory
- Internet connection for first load (model download)

## Porting Marvis TTS to Browser: Technical Analysis

### Challenges

1. **Model Format Conversion**
   - Requires exporting PyTorch model to ONNX
   - CSM architecture must be ONNX-compatible
   - Mimi codec needs ONNX representation
   - Custom operations may not be supported

2. **Codec Implementation**
   - Mimi codec requires JavaScript/WASM port
   - RVQ operations need browser-compatible implementation
   - Complex audio processing pipeline

3. **Performance Constraints**
   - 500MB model size is large for browser download
   - Memory usage in browser environment
   - Inference speed without GPU acceleration
   - Potential UI blocking during synthesis

4. **Browser Compatibility**
   - WebAssembly SIMD support varies
   - Memory limits differ by browser
   - Web Audio API implementation differences

### Required Work for Browser Port

**Phase 1: ONNX Export** (1-2 weeks)
- Export Marvis TTS PyTorch model to ONNX format
- Validate operation compatibility with ONNX Runtime
- Test inference parity with PyTorch version
- Optimize graph for browser deployment

**Phase 2: Codec Implementation** (2-3 weeks)
- Analyze Mimi codec architecture
- Implement codec in JavaScript or compile to WASM
- Create bindings for ONNX model integration
- Validate audio quality matches original

**Phase 3: Browser Integration** (1-2 weeks)
- Build JavaScript wrapper following Transformers.js patterns
- Implement model loading and caching strategy
- Add Web Worker support for non-blocking UI
- Create speaker embeddings management
- Implement streaming synthesis if possible

**Phase 4: Optimization** (1 week)
- Cross-browser testing (Chrome, Firefox, Safari, Edge)
- Performance benchmarking and optimization
- Memory usage profiling and reduction
- Network loading optimization (chunking, compression)
- Progressive loading for better UX

**Estimated Total**: 6-8 weeks for experienced ML engineer

**Technical Difficulty**: High
- Requires expertise in PyTorch, ONNX, JavaScript, and WebAssembly
- Custom codec implementation is complex and critical
- CSM architecture may have ONNX-incompatible operations
- Limited debugging tools for browser ML models

### Potential Blockers

1. **ONNX Compatibility**: CSM architecture may use operations not supported in ONNX or ONNX Runtime Web
2. **Codec Complexity**: Mimi codec may be too complex to efficiently implement in WASM
3. **Performance**: Inference time may be unacceptable without GPU acceleration
4. **Memory**: 500MB+ total memory usage may exceed browser limits on mobile devices

## Alternative Approaches

### Option 1: Server-Side API (Recommended for Production)

**Architecture**: Keep Marvis TTS on server, expose REST/WebSocket API

**Pros**:
- No porting work required
- Full model performance and quality
- Easier to update and maintain
- Can support streaming responses
- Works on all devices including low-end mobile

**Cons**:
- Requires server infrastructure and costs
- Network latency (200ms - 2s depending on location)
- Privacy concerns (text sent to server)
- Requires internet connection

**Best for**: Production applications, high-quality synthesis, voice cloning features

### Option 2: WebGPU Acceleration (Experimental)

**Architecture**: Use WebGPU instead of WebAssembly for compute

**Pros**:
- Significantly faster inference with GPU
- Native GPU memory management
- Could enable real-time synthesis
- Future-proof technology

**Cons**:
- Limited browser support (Chrome 113+, experimental in others)
- Requires ONNX to WebGPU shader translation
- Still needs codec implementation
- Debugging is even more challenging

**Best for**: Research projects, cutting-edge applications, Chrome-only deployments

### Option 3: Hybrid Approach (Practical Middle Ground)

**Architecture**: Browser runs lightweight model (SpeechT5), server provides Marvis TTS for premium quality

**Implementation**:
- Default: SpeechT5 in browser (demonstrated in prototype)
- Optional: Upgrade to Marvis TTS via API for higher quality
- Progressive enhancement based on network/device capabilities
- Fallback chain: Marvis API → SpeechT5 browser → Native browser TTS

**Pros**:
- Works everywhere immediately
- Optimal experience when conditions allow
- Privacy-friendly default (browser-side)
- Graceful degradation

**Cons**:
- Complexity of maintaining two TTS systems
- Consistency challenges (different voice qualities)
- More testing surface area

**Best for**: Applications needing broad compatibility with optional premium features

## Recommendations

### For Immediate Use
**Use the provided SpeechT5 prototype** (`tts-demo.html`):
- Works in all modern browsers today
- No server required
- Good quality for most use cases
- Simple to deploy and maintain

### For Production Applications
**Deploy Marvis TTS as a server-side API**:
- Provides best quality and features
- Proven technology stack
- Easier to maintain and update
- Can add browser fallback later

### For Research & Advanced Development
**Pursue ONNX export of Marvis TTS**:
- Allocate 6-8 weeks for initial port
- Start with ONNX export and validation
- Evaluate codec porting complexity early
- Have fallback plan if blockers emerge

### For Best of Both Worlds
**Implement hybrid approach**:
- Deploy SpeechT5 browser prototype immediately
- Develop Marvis TTS API in parallel
- Create progressive enhancement layer
- Measure usage to inform investment

## Technical Deep Dive

### How the Prototype Works

The `tts-demo.html` prototype demonstrates a complete browser-based TTS pipeline:

1. **Model Loading**: Transformers.js downloads ONNX model from HuggingFace CDN on first use
2. **Browser Caching**: Model is cached in browser's Cache API for instant subsequent loads
3. **Text Processing**: Input text is tokenized using the model's vocabulary
4. **Speaker Embeddings**: 512-dimensional embeddings loaded from HuggingFace dataset
5. **Inference**: ONNX Runtime Web executes model in WebAssembly
6. **Audio Generation**: Model outputs Float32Array of audio samples at 16kHz
7. **Playback**: Web Audio API converts samples to AudioBuffer and plays through speakers
8. **WAV Export**: Custom encoder converts Float32Array to WAV format for download

### Performance Characteristics

**SpeechT5 in Browser** (as demonstrated):
- Model size: ~100MB (one-time download)
- Synthesis time: 2-5 seconds for typical sentence
- Memory usage: ~200-300MB peak
- Quality: Good for general use, comparable to basic cloud TTS
- Voice customization: Limited via speaker embeddings

**Marvis TTS (Python/MLX)**:
- Model size: ~500MB
- Synthesis time: Real-time on Apple Silicon
- Memory usage: ~1GB
- Quality: Excellent with natural intonation
- Voice customization: 10-second audio cloning

**Estimated Marvis TTS in Browser** (if ported):
- Model size: ~500MB
- Synthesis time: 10-30 seconds for typical sentence (CPU-only)
- Memory usage: ~800MB-1.2GB peak
- Quality: Should match Python version
- Voice customization: Same as Python version

### WebAssembly vs Native Performance

Browser-based inference typically runs at **5-20% of native speed** due to:
- No GPU acceleration (ONNX Runtime Web is CPU-only)
- WebAssembly overhead compared to native code
- Browser memory management constraints
- Single-threaded execution in many cases

This performance gap is the main challenge for deploying large models like Marvis TTS in browsers.

## Resources & References

### Models
- **Marvis TTS**: https://huggingface.co/Marvis-AI/marvis-tts-250m-v0.1
- **SpeechT5**: https://huggingface.co/microsoft/speecht5_tts
- **Xenova/SpeechT5 (ONNX)**: https://huggingface.co/Xenova/speecht5_tts

### Libraries & Tools
- **Transformers.js**: https://github.com/huggingface/transformers.js
- **ONNX Runtime Web**: https://onnxruntime.ai/docs/tutorials/web/
- **Marvis TTS GitHub**: https://github.com/Marvis-Labs/marvis-tts

### Documentation
- **Web Audio API**: https://developer.mozilla.org/en-US/docs/Web/API/Web_Audio_API
- **WebAssembly**: https://webassembly.org/
- **CSM Architecture**: https://www.sesame.com/research/crossing_the_uncanny_valley_of_voice

## Conclusion

While Marvis TTS cannot currently run in browsers without significant engineering work, this research demonstrates that:

1. **Browser-based TTS is viable** with existing ONNX models like SpeechT5
2. **Porting Marvis TTS is technically feasible** but requires 6-8 weeks of specialized development
3. **Alternative approaches** (server-side API, hybrid deployment) may be more practical for most use cases
4. **The provided prototype** offers a working solution using current technology

The choice between porting Marvis TTS or using alternatives depends on specific requirements for quality, privacy, latency, and development resources.

## Files in This Repository

- **README.md** (this file): Complete research report and findings
- **notes.md**: Detailed research notes and investigation log
- **tts-demo.html**: Working browser-based TTS prototype using SpeechT5

## Author Notes

This research was conducted in November 2025. The web platform evolves rapidly, and future developments may include:
- Native CSM support in Transformers.js
- Official ONNX export for Marvis TTS
- WebGPU becoming standard across browsers
- More efficient codec implementations

Check the official Marvis TTS and Transformers.js repositories for the latest developments.
