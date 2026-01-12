Showcasing the versatility of the [whenwords](https://github.com/dbreunig/whenwords) time formatting specification, this project features parallel implementations in three esoteric programming languages: LOLCODE, Rockstar, and WebAssembly Text (WAT). Each version adapts the time formatting logic—such as "3 hours ago" and duration parsing—using the idiomatic constructs and limitations of its language, producing transpiled or compiled code for JavaScript, Python, or a compact WASM binary. All implementations were rigorously tested, passing 98.4% of cases, with minor edge-case discrepancies at month boundaries. Notably, the WAT code is available as a tiny 876-byte WASM with an [interactive playground](https://github.com/dbreunig/whenwords/tree/main/wat/playground.html), making these esoteric implementations accessible for experimentation and learning.

**Key findings/results:**
- All three languages successfully implement core functions (`timeago`, `duration`) as defined in the whenwords spec.
- Testing reveals only two edge-case rounding errors with month calculations.
- The WASM binary is highly compact and includes a demo for hands-on testing.
- Implementation strategies varied to accommodate each language's capabilities and transpiler limitations.
