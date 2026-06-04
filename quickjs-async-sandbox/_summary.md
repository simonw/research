Exploring the [`quickjs`](https://pypi.org/project/quickjs/) Python package, this project implements an asyncio-compatible JavaScript sandbox with robust resource controls and seamless exposure of both synchronous and asynchronous Python functions (including async httpx fetches) to JavaScript code. The investigation verified critical sandbox features: hard memory caps, reliable wall-clock execution limits, concurrency, and safe async bridging — but also revealed three key constraints in QuickJS’s threading and callback model, shaping how the sandbox enforces timeouts and handles exceptions. For adversarial inputs, a process-based variant (`ProcessQuickJSSandbox`, see [sandbox_process.py](https://github.com/the-user/your-repo/blob/main/sandbox_process.py)) guarantees hard termination and isolation, albeit at higher start-up cost. The thread-based approach (`AsyncQuickJSSandbox`) is fast and sufficient for trusted-ish plugin code, with comprehensive example scenarios and caveats documented.

**Key findings:**
- Hard memory and wall-clock limits are enforced reliably; exposed Python functions (sync and async) are safely callable from JavaScript.
- QuickJS cannot interrupt a running `eval` from another thread, and built-in time limits break when callbacks are used; timeouts must be enforced externally.
- Python exceptions inside callbacks corrupt the JS context, necessitating error-wrapping techniques for callable exposure.
- Process sandbox is necessary for true termination safety with untrusted code; thread-based sandbox offers ~10× faster runs but may leak threads.
- JS-side concurrency (Promise fan-out) is limited by design in the thread sandbox; a more complex deferred resolution pattern is possible but not implemented for production.

**Useful tools:**
- [quickjs PyPI package](https://pypi.org/project/quickjs/)
- Process-based sandbox example: `sandbox_process.py` (link depends on your repo)
