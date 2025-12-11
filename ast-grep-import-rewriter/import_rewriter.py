#!/usr/bin/env python3
"""
AST-Grep Import Rewriter

A tool that uses ast-grep to parse JavaScript files and rewrite obfuscated
module import references based on a mapping file.

Usage:
    python import_rewriter.py <input_file> --mapping <mapping_file> [--output <output_file>]
    python import_rewriter.py <input_file> --extract  # Extract all imports to stdout
    python import_rewriter.py <input_file> --extract --format json  # Extract as JSON
"""

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


@dataclass
class ImportMatch:
    """Represents a matched import in the source code."""
    line: int
    column: int
    end_line: int
    end_column: int
    text: str
    import_type: str
    source_path: str


def run_ast_grep(pattern: str, file_path: str, lang: str = "javascript") -> str:
    """Run ast-grep with a pattern and return the output."""
    cmd = [
        "ast-grep",
        "run",
        "--pattern", pattern,
        "--lang", lang,
        "--json",
        str(file_path)
    ]
    result = subprocess.run(cmd, capture_output=True, text=True)
    return result.stdout


def run_ast_grep_rewrite(pattern: str, rewrite: str, file_path: str,
                         lang: str = "javascript", dry_run: bool = True) -> str:
    """Run ast-grep with a pattern and rewrite, return the diff or apply changes."""
    cmd = [
        "ast-grep",
        "run",
        "--pattern", pattern,
        "--rewrite", rewrite,
        "--lang", lang,
        str(file_path)
    ]
    if not dry_run:
        cmd.insert(2, "--update-all")

    result = subprocess.run(cmd, capture_output=True, text=True)
    return result.stdout


def extract_source_from_import(import_text: str) -> Optional[str]:
    """Extract the module source path from an import statement."""
    # Match both single and double quoted strings
    patterns = [
        r'from\s+["\']([^"\']+)["\']',  # ES6 imports
        r'require\s*\(\s*["\']([^"\']+)["\']\s*\)',  # CommonJS require
        r'import\s*\(\s*["\']([^"\']+)["\']\s*\)',  # Dynamic import
        r'__webpack_require__\s*\(\s*["\']([^"\']+)["\']\s*\)',  # Webpack require
    ]

    for pattern in patterns:
        match = re.search(pattern, import_text)
        if match:
            return match.group(1)
    return None


def find_imports(file_path: str) -> list[ImportMatch]:
    """Find all imports in a JavaScript file using ast-grep."""
    imports = []

    # Patterns to search for different import types
    patterns = [
        ('import $$$ALL', 'es6'),
        ('require("$SOURCE")', 'commonjs'),
        ("require('$SOURCE')", 'commonjs-single'),
        ('import("$SOURCE")', 'dynamic'),
        ("import('$SOURCE')", 'dynamic-single'),
        ('export { $$$NAMES } from "$SOURCE"', 'reexport'),
        ("export { $$$NAMES } from '$SOURCE'", 'reexport-single'),
        ('export * from "$SOURCE"', 'reexport-all'),
        ("export * from '$SOURCE'", 'reexport-all-single'),
        # Webpack-specific patterns
        ('__webpack_require__("$SOURCE")', 'webpack'),
        ("__webpack_require__('$SOURCE')", 'webpack-single'),
    ]

    seen_positions = set()

    for pattern, import_type in patterns:
        output = run_ast_grep(pattern, file_path)
        if output.strip():
            try:
                matches = json.loads(output)
                for match in matches:
                    text = match.get('text', '')
                    range_info = match.get('range', {})
                    start = range_info.get('start', {})
                    end = range_info.get('end', {})

                    # Create a position key to avoid duplicates
                    pos_key = (start.get('line', 0), start.get('column', 0))
                    if pos_key in seen_positions:
                        continue
                    seen_positions.add(pos_key)

                    source_path = extract_source_from_import(text)
                    if source_path:
                        imports.append(ImportMatch(
                            line=start.get('line', 0),
                            column=start.get('column', 0),
                            end_line=end.get('line', 0),
                            end_column=end.get('column', 0),
                            text=text,
                            import_type=import_type,
                            source_path=source_path
                        ))
            except json.JSONDecodeError:
                pass

    # Sort by line number
    imports.sort(key=lambda x: (x.line, x.column))
    return imports


def load_mapping(mapping_file: str) -> dict[str, str]:
    """Load the import mapping from a JSON file."""
    with open(mapping_file, 'r') as f:
        return json.load(f)


def generate_mapping_from_pattern(imports: list[ImportMatch],
                                   pattern: str = r'_0x[a-f0-9]+') -> dict[str, str]:
    """Generate a suggested mapping file for obfuscated imports."""
    mapping = {}
    counter = 1

    for imp in imports:
        source = imp.source_path
        if re.search(pattern, source):
            # Generate a readable name
            # Extract base name without path and extension
            base = os.path.basename(source)
            name_without_ext = os.path.splitext(base)[0]

            # Create a suggested mapping
            dir_part = os.path.dirname(source)
            suggested_name = f"module_{counter}"

            if dir_part:
                mapping[source] = f"{dir_part}/{suggested_name}"
            else:
                mapping[source] = suggested_name
            counter += 1

    return mapping


def rewrite_file_with_mapping(file_path: str, mapping: dict[str, str],
                               output_path: Optional[str] = None) -> str:
    """Rewrite the file using the provided mapping."""
    # Read the original file
    with open(file_path, 'r') as f:
        content = f.read()

    # Track changes for reporting
    changes = []

    # Apply each mapping
    for old_path, new_path in mapping.items():
        if old_path in content:
            # Escape for regex
            escaped_old = re.escape(old_path)
            # Replace both single and double quoted versions
            content, count = re.subn(f'(["\']){escaped_old}(["\'])',
                                     f'\\g<1>{new_path}\\g<2>', content)
            if count > 0:
                changes.append(f"  {old_path} -> {new_path} ({count} occurrences)")

    # Write to output file or return
    if output_path:
        with open(output_path, 'w') as f:
            f.write(content)
        return f"Wrote {len(changes)} changes to {output_path}:\n" + "\n".join(changes)
    else:
        return content


def rewrite_with_ast_grep(file_path: str, mapping: dict[str, str],
                          output_path: Optional[str] = None,
                          dry_run: bool = True) -> str:
    """Rewrite imports using ast-grep for more precise AST-based replacement."""
    results = []

    # Copy file to temp location if we're not doing dry run
    if not dry_run:
        if output_path:
            # Copy to output path
            with open(file_path, 'r') as f:
                content = f.read()
            with open(output_path, 'w') as f:
                f.write(content)
            work_file = output_path
        else:
            work_file = file_path
    else:
        work_file = file_path

    # Apply each mapping using ast-grep
    for old_path, new_path in mapping.items():
        # Pattern variations for different quote styles and import types
        rewrites = [
            # ES6 imports with double quotes
            (f'import $CLAUSE from "{old_path}"', f'import $CLAUSE from "{new_path}"'),
            # ES6 imports with single quotes
            (f"import $CLAUSE from '{old_path}'", f"import $CLAUSE from '{new_path}'"),
            # CommonJS require double quotes
            (f'require("{old_path}")', f'require("{new_path}")'),
            # CommonJS require single quotes
            (f"require('{old_path}')", f"require('{new_path}')"),
            # Dynamic import double quotes
            (f'import("{old_path}")', f'import("{new_path}")'),
            # Dynamic import single quotes
            (f"import('{old_path}')", f"import('{new_path}')"),
            # Re-exports double quotes
            (f'export $CLAUSE from "{old_path}"', f'export $CLAUSE from "{new_path}"'),
            # Re-exports single quotes
            (f"export $CLAUSE from '{old_path}'", f"export $CLAUSE from '{new_path}'"),
            # Webpack require double quotes
            (f'__webpack_require__("{old_path}")', f'__webpack_require__("{new_path}")'),
            # Webpack require single quotes
            (f"__webpack_require__('{old_path}')", f"__webpack_require__('{new_path}')"),
        ]

        for pattern, rewrite in rewrites:
            output = run_ast_grep_rewrite(pattern, rewrite, work_file,
                                         dry_run=dry_run)
            if output.strip():
                results.append(f"Pattern: {pattern}")
                results.append(output)

    return "\n".join(results) if results else "No changes needed"


def main():
    parser = argparse.ArgumentParser(
        description="Rewrite obfuscated JavaScript imports using ast-grep"
    )
    parser.add_argument("input_file", help="JavaScript file to process")
    parser.add_argument("--mapping", "-m", help="JSON file with import mapping")
    parser.add_argument("--output", "-o", help="Output file (default: modify in place)")
    parser.add_argument("--extract", "-e", action="store_true",
                        help="Extract all imports from the file")
    parser.add_argument("--format", "-f", choices=["text", "json"], default="text",
                        help="Output format for --extract")
    parser.add_argument("--generate-mapping", "-g", action="store_true",
                        help="Generate a template mapping file for obfuscated imports")
    parser.add_argument("--pattern", "-p", default=r'_0x[a-f0-9]+',
                        help="Regex pattern to identify obfuscated imports")
    parser.add_argument("--dry-run", "-n", action="store_true",
                        help="Show what would be changed without modifying files")
    parser.add_argument("--use-ast", action="store_true",
                        help="Use ast-grep for rewriting (more precise but slower)")

    args = parser.parse_args()

    if not os.path.exists(args.input_file):
        print(f"Error: File not found: {args.input_file}", file=sys.stderr)
        sys.exit(1)

    # Extract imports mode
    if args.extract:
        imports = find_imports(args.input_file)
        if args.format == "json":
            output = [
                {
                    "line": imp.line,
                    "column": imp.column,
                    "type": imp.import_type,
                    "source": imp.source_path,
                    "text": imp.text
                }
                for imp in imports
            ]
            print(json.dumps(output, indent=2))
        else:
            print(f"Found {len(imports)} imports in {args.input_file}:\n")
            for imp in imports:
                print(f"  Line {imp.line}: [{imp.import_type}] {imp.source_path}")
                print(f"    {imp.text[:80]}{'...' if len(imp.text) > 80 else ''}")
                print()
        return

    # Generate mapping mode
    if args.generate_mapping:
        imports = find_imports(args.input_file)
        mapping = generate_mapping_from_pattern(imports, args.pattern)
        print(json.dumps(mapping, indent=2))
        return

    # Rewrite mode - requires mapping file
    if not args.mapping:
        print("Error: --mapping file is required for rewriting", file=sys.stderr)
        print("Use --extract to see all imports, or --generate-mapping to create a template",
              file=sys.stderr)
        sys.exit(1)

    if not os.path.exists(args.mapping):
        print(f"Error: Mapping file not found: {args.mapping}", file=sys.stderr)
        sys.exit(1)

    mapping = load_mapping(args.mapping)

    if args.use_ast:
        result = rewrite_with_ast_grep(
            args.input_file,
            mapping,
            args.output,
            dry_run=args.dry_run
        )
        print(result)
    else:
        if args.dry_run:
            # Show diff-like output
            print("Dry run - changes that would be made:")
            result = rewrite_file_with_mapping(args.input_file, mapping)
            print("\n--- Original")
            print("+++ Modified")
            # Simple diff
            with open(args.input_file, 'r') as f:
                original = f.read()

            orig_lines = original.split('\n')
            new_lines = result.split('\n')

            for i, (orig, new) in enumerate(zip(orig_lines, new_lines), 1):
                if orig != new:
                    print(f"@@ Line {i} @@")
                    print(f"- {orig}")
                    print(f"+ {new}")
        else:
            output_path = args.output or args.input_file
            result = rewrite_file_with_mapping(args.input_file, mapping, output_path)
            print(result)


if __name__ == "__main__":
    main()
