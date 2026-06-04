REXC (rx) JSON Test Suite provides a comprehensive, language-agnostic test resource for validating implementations of the [REXC encoder/decoder](https://github.com/creationix/rx). It includes a single JSON file with 206 tests covering base64 encoding, zigzag integer transformations, value conversions, roundtrip integrity, and special numeric values, ensuring correctness across platforms. The suite is accompanied by a TypeScript runner utilizing Vitest and a standalone Python port fully tested via pytest, demonstrating cross-language fidelity and completeness. Both the TypeScript and Python implementations pass all test cases, verifying consistent and reliable encoding and decoding behavior.

**Key features:**
- JSON test suite covers all encoder/decoder edge cases and formats.
- [Python implementation](https://github.com/creationix/rx/tree/main/rx-python) passes all tests as a faithful port.
- TypeScript runner ensures the test suite remains synchronized with the reference implementation.
- Supports advanced encoding features (schemas, pointers, special values).
