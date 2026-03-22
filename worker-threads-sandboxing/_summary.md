Examining Node.js v22, this research details the built-in and third-party sandboxing mechanisms available for executing untrusted or semi-trusted JavaScript code. Node’s worker_threads module enables environmental isolation, resource limits, and the use of the Node Permission Model (now stable in v22.13.0), which significantly restricts filesystem, process, and other OS-level capabilities when used via `execArgv`. While the `vm` module provides global scope isolation, it is not secure against malicious payloads due to prototype escapes—but combined with Worker permission settings, those escapes are neutered. For stronger in-process boundaries, [`isolated-vm`](https://www.npmjs.com/package/isolated-vm) offers V8 isolate-based separation, while [`quickjs-emscripten`](https://www.npmjs.com/package/quickjs-emscripten) uses WebAssembly and a separate JS engine for robust isolation. True security for adversarial code requires OS- or container-level sandboxing in addition to Node solutions.

**Key Findings:**
- Node.js Permission Model is stable (v22+) and can be enforced per-worker via `execArgv`.
- Prototype escapes in `vm` are blocked operationally when the permission model denies OS resource access.
- Resource limits protect only the V8 heap, not external/native allocations.
- Avoid `vm2` for security; prefer `isolated-vm` or `quickjs-emscripten` for untrusted code.
- Ultimate security requires running sandboxed logic in a subprocess or container with OS-level hardening.
