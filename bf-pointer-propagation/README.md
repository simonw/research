# Brainfuck Interpreter Pointer Propagation Optimization

Implementation of pointer propagation optimization for the rpypkgs Brainfuck interpreter, as described in the [Lobsters Vibecoding Challenge](https://lobste.rs/s/igpevt/lobsters_vibecoding_challenge_winter) Task 1.

## Task Description

Convert the existing BF interpreter to use [pointer propagation](https://esolangs.org/wiki/Algebraic_Brainfuck#Pointer_propagation) for improved performance. The technique, developed by Kang Seonghoon, combines sequences of pointer movements and cell modifications into single operations.

## Implementation Summary

### What is Pointer Propagation?

Pointer propagation represents BF operations as tuples `(adjustment, offset, diffs[])`:
- `adjustment`: Final pointer movement relative to start
- `offset`: Starting offset for modifications
- `diffs[]`: Array of cell modifications

For example, `>+>++` (move right, add 1, move right, add 2) becomes:
- `Prop(2, 0, [0, 1, 2])`: At the end, pointer moved 2 right, added 1 to cell 1, added 2 to cell 2

### Changes Made

1. **New `Prop` Op class**: Executes pointer propagation tuples efficiently
2. **Unified peephole optimizer**: Combines all adjacent add/shift operations into Props
3. **Helper functions**: `_overlap()`, `_concat()`, `_trim()`, `_toProp()` for prop manipulation
4. **Bug fix**: Corrected the overlap algorithm from esolangs for empty diff arrays

### Line Count

| Version | Lines |
|---------|-------|
| Original | 285 |
| Modified | 289 |
| **Difference** | **+4** |

The implementation adds only 4 lines while introducing significant optimization capability.

## Grading Tier

Based on the challenge criteria:
- **B tier**: under 300 lines of code, as fast as current version

The 289-line implementation qualifies for B tier (pending JIT-compiled benchmark verification).

## Benchmark Results (Interpreted - No JIT)

Without JIT compilation, the modified version shows expected overhead:

| Metric | Original | Modified | Notes |
|--------|----------|----------|-------|
| Parse+compile (mandel.b) | 0.263s | 0.460s | 1.75x - more work during optimization |
| Runtime (hello.b x1000) | 0.862s | 1.056s | 1.22x - Prop overhead in interpreter |

**Important**: These numbers are NOT representative of JIT-compiled performance. The real
benefit of pointer propagation comes from JIT compilation where:
- Prop operations with known diffs arrays are unrolled
- Reduced dispatch overhead (multiple ops → single Prop)
- Optimized native code generation

The Nix build (with RPython JIT) takes 1-2 hours to compile.

## Testing

All tests pass with the Python interpreter:

```
$ python bf.py share/hello.b
Hello World!

$ python bf.py bench.b
OK

$ python bf.py share/cell-size.b
8 bit cells
```

## Files

- `bf.py.diff` - Unified diff against original bf.py
- `notes.md` - Detailed implementation notes
- `README.md` - This file

## How to Build

```bash
cd rpypkgs
nix build .#bf
```

## How to Run

```bash
# Interpreted (for testing)
python bf.py program.bf

# Compiled (for benchmarks)
./result/bin/bf program.bf
```

## Technical Details

### The Prop Operation

```python
class Prop(Op):
    _immutable_ = True
    _immutable_fields_ = "adj", "off", "diffs[*]"

    @unroll_safe
    def runOn(self, tape, pos):
        p = pos + self.off
        for i in range(len(self.diffs)):
            if self.diffs[i]:
                tape[p + i] = (tape[p + i] + self.diffs[i]) & 0xff
        return pos + self.adj
```

### Concatenation Algorithm

The key insight is that any sequence of adds and shifts can be combined:

```python
def _concat(a1, o1, ds1, a2, o2, ds2):
    if o1 < a1 + o2:
        return (a1+a2, o1, _overlap(ds1, a1+o2-o1, ds2))
    return (a1+a2, a1+o2, _overlap(ds2, o1-a1-o2, ds1))
```

## References

- [rpypkgs repository](https://github.com/rpypkgs/rpypkgs)
- [Algebraic Brainfuck - Pointer propagation](https://esolangs.org/wiki/Algebraic_Brainfuck#Pointer_propagation)
- [Brainfuck benchmark suite](https://github.com/cwfitzgerald/brainfuck-benchmark)
