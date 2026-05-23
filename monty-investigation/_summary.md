Reviewing [`pydantic-monty`](https://github.com/pydantic/monty) reveals it as a fast, minimal Python interpreter designed for controlled sandboxed execution, primarily useful when transforming data, branching, looping, and interacting with a select set of trusted host tools or a virtual filesystem. The interpreter purposefully omits large portions of CPython’s functionality, with clear boundaries: unsupported features and missing resources generally fail cleanly as structured errors rather than escaping into the host runtime. Security hinges on strict isolation—Monty code can’t directly access host resources except via explicit, trusted callbacks, which are outside the sandbox and should be tightly scoped. Resource limits (duration, memory, allocations, recursion) were reliably enforced, blocking runaway code, and virtual filesystem mounts behaved as expected, with overlay and sandbox modes.

Key findings:
- Unexpectedly wide subset of basic Python works: arithmetic, control flow, builtins, `sys`, `math`, `re`, and more (see [source tests](https://github.com/pydantic/monty/tree/main/tests)).
- Many dynamic, introspective, and advanced features are missing: classes, context managers, real imports, and dynamic introspection.
- Virtual filesystems allow controlled reads/writes; host access is only possible via explicitly-exported callbacks.
- F-string formatting flags in package version `0.0.17` showed silent omissions not aligned with source-level stricter parsing—future upgrades may tighten behavior.
- Type checking and snapshot pause/resume work, supporting async external calls and stateful agent workflows.
- Resource limits and sandboxing were robust, catching infinite loops, excessive allocations, and host access attempts.
