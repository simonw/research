"""
Performance benchmark for Python markdown libraries
Compares cmarkgfm against other popular markdown parsers
"""

import time
import statistics
import sys
from typing import Callable, Dict, List, Tuple

# Test documents of various sizes and features
TEST_DOCUMENTS = {
    "small_basic": """# Hello World

This is a **bold** statement and this is *italic*.

Here's a [link](https://example.com).""",

    "medium_mixed": """# Markdown Test Document

## Headers and Paragraphs

This is a paragraph with **bold text**, *italic text*, and ***bold italic***.
Here's some `inline code` and a [link to Python](https://python.org).

### Lists

Unordered list:
- Item 1
- Item 2
  - Nested item 2.1
  - Nested item 2.2
- Item 3

Ordered list:
1. First item
2. Second item
3. Third item

### Code Blocks

```python
def hello_world():
    print("Hello, World!")
    return 42
```

### Blockquotes

> This is a blockquote.
> It can span multiple lines.
>
> > Nested blockquote

### Links and Images

[Python](https://python.org) is great.
![Alt text](https://example.com/image.png)
""",

    "large_complex": """# Large Markdown Document

## Introduction

This document contains various markdown elements to test parsing performance.
It includes **bold**, *italic*, ***bold italic***, `code`, and [links](https://example.com).

## Tables

| Library | Speed | Features | Language |
|---------|-------|----------|----------|
| cmarkgfm | Very Fast | GFM | C + Python |
| mistune | Fast | Plugins | Python |
| markdown | Moderate | Extensions | Python |
| markdown2 | Moderate | Extras | Python |

## Long Lists

### Unordered List

""" + "\n".join([f"- List item number {i} with some text content" for i in range(50)]) + """

### Ordered List

""" + "\n".join([f"{i}. Ordered item number {i} with some content" for i in range(1, 51)]) + """

## Code Blocks

```python
class MarkdownParser:
    def __init__(self, text):
        self.text = text

    def parse(self):
        # Complex parsing logic here
        tokens = self.tokenize()
        ast = self.build_ast(tokens)
        return self.render(ast)

    def tokenize(self):
        return []

    def build_ast(self, tokens):
        return {}

    def render(self, ast):
        return "<html></html>"
```

```javascript
function processMarkdown(text) {
    const lines = text.split('\\n');
    return lines.map(line => {
        if (line.startsWith('#')) {
            return `<h1>${line.slice(1)}</h1>`;
        }
        return `<p>${line}</p>`;
    }).join('\\n');
}
```

## Nested Structures

> This is a blockquote with **bold** text.
>
> > Nested blockquote with *italic*.
> >
> > - List item in blockquote
> > - Another list item
> >   - Deeply nested item
>
> Back to first level.

## Multiple Paragraphs

""" + "\n\n".join([f"Paragraph number {i}. Lorem ipsum dolor sit amet, consectetur adipiscing elit. "
                    f"Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. "
                    f"Ut enim ad minim veniam, quis nostrud exercitation ullamco."
                    for i in range(20)]) + """

## Links and References

Here are some [inline links](https://example.com) and more [links to other sites](https://github.com).

[Reference link][ref1]

More text here.

[Another reference][ref2]

[ref1]: https://example.com/page1
[ref2]: https://example.com/page2

## Emphasis and Strong

This paragraph contains *single asterisk italic*, _single underscore italic_,
**double asterisk bold**, __double underscore bold__, and ***triple asterisk bold italic*__.

## Horizontal Rules

---

***

___

## End

That's all folks!
"""
}


def benchmark_function(func: Callable, text: str, iterations: int = 100) -> Dict[str, float]:
    """Benchmark a markdown rendering function"""
    times = []

    # Warmup
    for _ in range(5):
        func(text)

    # Actual benchmark
    for _ in range(iterations):
        start = time.perf_counter()
        func(text)
        end = time.perf_counter()
        times.append((end - start) * 1000)  # Convert to milliseconds

    return {
        'mean': statistics.mean(times),
        'median': statistics.median(times),
        'stdev': statistics.stdev(times) if len(times) > 1 else 0,
        'min': min(times),
        'max': max(times)
    }


def get_library_functions() -> Dict[str, Tuple[Callable, str, str]]:
    """Get all available markdown library functions with their info"""
    functions = {}

    # cmarkgfm
    try:
        import cmarkgfm
        functions['cmarkgfm'] = (
            lambda text: cmarkgfm.github_flavored_markdown_to_html(text),
            "C-based (GitHub's cmark fork)",
            "GFM"
        )
    except ImportError:
        print("Warning: cmarkgfm not installed", file=sys.stderr)

    # markdown (Python-Markdown)
    try:
        import markdown
        functions['markdown'] = (
            lambda text: markdown.markdown(text),
            "Pure Python",
            "Classic"
        )
    except ImportError:
        print("Warning: markdown not installed", file=sys.stderr)

    # mistune
    try:
        import mistune
        functions['mistune'] = (
            lambda text: mistune.html(text),
            "Pure Python",
            "v3"
        )
    except ImportError:
        print("Warning: mistune not installed", file=sys.stderr)

    # markdown2
    try:
        import markdown2
        functions['markdown2'] = (
            lambda text: markdown2.markdown(text),
            "Pure Python",
            "Extras"
        )
    except ImportError:
        print("Warning: markdown2 not installed", file=sys.stderr)

    # commonmark
    try:
        import commonmark
        functions['commonmark'] = (
            lambda text: commonmark.commonmark(text),
            "Pure Python (deprecated)",
            "CommonMark"
        )
    except ImportError:
        print("Warning: commonmark not installed", file=sys.stderr)

    # marko
    try:
        import marko
        functions['marko'] = (
            lambda text: marko.convert(text),
            "Pure Python",
            "CommonMark 0.31.2"
        )
    except ImportError:
        print("Warning: marko not installed", file=sys.stderr)

    # mistletoe
    try:
        import mistletoe
        functions['mistletoe'] = (
            lambda text: mistletoe.markdown(text),
            "Pure Python",
            "CommonMark"
        )
    except ImportError:
        print("Warning: mistletoe not installed", file=sys.stderr)

    return functions


def run_benchmarks() -> Dict:
    """Run all benchmarks and return results"""
    libraries = get_library_functions()

    if not libraries:
        print("Error: No markdown libraries installed!", file=sys.stderr)
        sys.exit(1)

    print(f"\nBenchmarking {len(libraries)} markdown libraries...")
    print(f"Libraries: {', '.join(libraries.keys())}\n")

    results = {}

    for doc_name, doc_text in TEST_DOCUMENTS.items():
        print(f"Testing {doc_name} ({len(doc_text)} chars)...")
        results[doc_name] = {}

        for lib_name, (func, impl, spec) in libraries.items():
            try:
                print(f"  {lib_name}...", end=" ", flush=True)
                bench_results = benchmark_function(func, doc_text, iterations=100)
                results[doc_name][lib_name] = bench_results
                print(f"{bench_results['mean']:.2f}ms")
            except Exception as e:
                print(f"ERROR: {e}")
                results[doc_name][lib_name] = None

    return results, libraries


def print_results(results: Dict, libraries: Dict):
    """Print benchmark results in a formatted table"""
    print("\n" + "=" * 80)
    print("BENCHMARK RESULTS")
    print("=" * 80)

    for doc_name, doc_results in results.items():
        print(f"\n{doc_name.upper()}:")
        print("-" * 80)

        # Sort by mean time
        sorted_results = sorted(
            [(lib, res) for lib, res in doc_results.items() if res],
            key=lambda x: x[1]['mean']
        )

        for lib_name, bench_results in sorted_results:
            impl, spec = libraries[lib_name][1:]
            print(f"{lib_name:15} {bench_results['mean']:8.2f}ms ± {bench_results['stdev']:6.2f}ms "
                  f"[{bench_results['min']:7.2f} - {bench_results['max']:7.2f}] "
                  f"({impl})")


def save_results(results: Dict, libraries: Dict):
    """Save results to JSON file"""
    import json

    # Add library info to results
    output = {
        'libraries': {
            name: {'implementation': impl, 'spec': spec}
            for name, (_, impl, spec) in libraries.items()
        },
        'results': results
    }

    with open('/home/user/research/python-markdown-comparison/benchmark_results.json', 'w') as f:
        json.dump(output, f, indent=2)

    print("\nResults saved to benchmark_results.json")


if __name__ == '__main__':
    results, libraries = run_benchmarks()
    print_results(results, libraries)
    save_results(results, libraries)
