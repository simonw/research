"""
Tests for the ast-grep import rewriter tool.

Run with: pytest test_import_rewriter.py -v
"""

import json
import os
import tempfile
from pathlib import Path

import pytest

from import_rewriter import (
    ImportMatch,
    extract_source_from_import,
    find_imports,
    generate_mapping_from_pattern,
    load_mapping,
    rewrite_file_with_mapping,
)


# =============================================================================
# Fixtures
# =============================================================================

@pytest.fixture
def temp_dir():
    """Create a temporary directory for test files."""
    with tempfile.TemporaryDirectory() as tmpdir:
        yield Path(tmpdir)


@pytest.fixture
def sample_es6_file(temp_dir):
    """Create a sample file with ES6 imports."""
    content = '''
import React from "react";
import { useState, useEffect } from "react";
import * as utils from "./utils_0x1234";
import defaultExport, { named } from "./module_0xabcd";
'''
    path = temp_dir / "es6_sample.js"
    path.write_text(content)
    return path


@pytest.fixture
def sample_commonjs_file(temp_dir):
    """Create a sample file with CommonJS requires."""
    content = '''
const fs = require("fs");
const path = require('path');
const utils = require("./utils_0x5678");
const { helper } = require("./helper_0x9abc");
'''
    path = temp_dir / "commonjs_sample.js"
    path.write_text(content)
    return path


@pytest.fixture
def sample_mixed_file(temp_dir):
    """Create a sample file with mixed import types."""
    content = '''
import defaultMod from "./default_0x1111";
import { named } from "./named_0x2222";
const required = require("./required_0x3333");
const dynamic = await import("./dynamic_0x4444");
export { something } from "./reexport_0x5555";
export * from "./reexport_all_0x6666";
'''
    path = temp_dir / "mixed_sample.js"
    path.write_text(content)
    return path


@pytest.fixture
def sample_webpack_file(temp_dir):
    """Create a sample file with webpack requires."""
    content = '''
var _utils = __webpack_require__("./utils_0xaaaa");
var _api = __webpack_require__("./api_0xbbbb");
const lodash = __webpack_require__("lodash");
'''
    path = temp_dir / "webpack_sample.js"
    path.write_text(content)
    return path


@pytest.fixture
def sample_mapping_file(temp_dir):
    """Create a sample mapping JSON file."""
    mapping = {
        "./utils_0x1234": "./utils/helpers",
        "./module_0xabcd": "./modules/main",
        "./default_0x1111": "./default-module",
    }
    path = temp_dir / "mapping.json"
    path.write_text(json.dumps(mapping))
    return path


# =============================================================================
# Tests for extract_source_from_import
# =============================================================================

class TestExtractSourceFromImport:
    """Tests for the extract_source_from_import function."""

    def test_es6_import_double_quotes(self):
        """Test extraction from ES6 import with double quotes."""
        text = 'import React from "react"'
        assert extract_source_from_import(text) == "react"

    def test_es6_import_single_quotes(self):
        """Test extraction from ES6 import with single quotes."""
        text = "import React from 'react'"
        assert extract_source_from_import(text) == "react"

    def test_es6_named_import(self):
        """Test extraction from ES6 named import."""
        text = 'import { useState } from "react"'
        assert extract_source_from_import(text) == "react"

    def test_es6_namespace_import(self):
        """Test extraction from ES6 namespace import."""
        text = 'import * as utils from "./utils"'
        assert extract_source_from_import(text) == "./utils"

    def test_commonjs_require_double_quotes(self):
        """Test extraction from CommonJS require with double quotes."""
        text = 'require("lodash")'
        assert extract_source_from_import(text) == "lodash"

    def test_commonjs_require_single_quotes(self):
        """Test extraction from CommonJS require with single quotes."""
        text = "require('lodash')"
        assert extract_source_from_import(text) == "lodash"

    def test_commonjs_require_with_assignment(self):
        """Test extraction from CommonJS require with variable assignment."""
        text = 'const _ = require("lodash")'
        assert extract_source_from_import(text) == "lodash"

    def test_dynamic_import_double_quotes(self):
        """Test extraction from dynamic import with double quotes."""
        text = 'import("./lazy-module")'
        assert extract_source_from_import(text) == "./lazy-module"

    def test_dynamic_import_single_quotes(self):
        """Test extraction from dynamic import with single quotes."""
        text = "import('./lazy-module')"
        assert extract_source_from_import(text) == "./lazy-module"

    def test_dynamic_import_await(self):
        """Test extraction from dynamic import with await."""
        text = 'await import("./lazy-module")'
        assert extract_source_from_import(text) == "./lazy-module"

    def test_webpack_require_double_quotes(self):
        """Test extraction from webpack require with double quotes."""
        text = '__webpack_require__("./module")'
        assert extract_source_from_import(text) == "./module"

    def test_webpack_require_single_quotes(self):
        """Test extraction from webpack require with single quotes."""
        text = "__webpack_require__('./module')"
        assert extract_source_from_import(text) == "./module"

    def test_obfuscated_path(self):
        """Test extraction with obfuscated module path."""
        text = 'import foo from "./mod_0x1a2b3c"'
        assert extract_source_from_import(text) == "./mod_0x1a2b3c"

    def test_relative_path(self):
        """Test extraction with relative path."""
        text = 'import utils from "../utils/helpers"'
        assert extract_source_from_import(text) == "../utils/helpers"

    def test_no_import_returns_none(self):
        """Test that non-import text returns None."""
        text = "console.log('hello')"
        assert extract_source_from_import(text) is None

    def test_empty_string_returns_none(self):
        """Test that empty string returns None."""
        assert extract_source_from_import("") is None

    def test_multiline_import(self):
        """Test extraction from multiline import."""
        text = '''import {
  foo,
  bar
} from "./module"'''
        assert extract_source_from_import(text) == "./module"


# =============================================================================
# Tests for find_imports
# =============================================================================

class TestFindImports:
    """Tests for the find_imports function."""

    def test_find_es6_imports(self, sample_es6_file):
        """Test finding ES6 imports in a file."""
        imports = find_imports(str(sample_es6_file))

        # Should find imports for react and the obfuscated modules
        sources = [imp.source_path for imp in imports]
        assert "react" in sources
        assert "./utils_0x1234" in sources
        assert "./module_0xabcd" in sources

    def test_find_commonjs_requires(self, sample_commonjs_file):
        """Test finding CommonJS requires in a file."""
        imports = find_imports(str(sample_commonjs_file))

        sources = [imp.source_path for imp in imports]
        assert "fs" in sources
        assert "path" in sources
        assert "./utils_0x5678" in sources
        assert "./helper_0x9abc" in sources

    def test_find_mixed_imports(self, sample_mixed_file):
        """Test finding mixed import types."""
        imports = find_imports(str(sample_mixed_file))

        sources = [imp.source_path for imp in imports]
        assert "./default_0x1111" in sources
        assert "./named_0x2222" in sources
        assert "./required_0x3333" in sources
        assert "./dynamic_0x4444" in sources
        assert "./reexport_0x5555" in sources
        assert "./reexport_all_0x6666" in sources

    def test_find_webpack_requires(self, sample_webpack_file):
        """Test finding webpack requires."""
        imports = find_imports(str(sample_webpack_file))

        sources = [imp.source_path for imp in imports]
        assert "./utils_0xaaaa" in sources
        assert "./api_0xbbbb" in sources
        assert "lodash" in sources

    def test_import_types_are_correct(self, sample_mixed_file):
        """Test that import types are correctly identified."""
        imports = find_imports(str(sample_mixed_file))

        # Find specific imports and check their types
        import_dict = {imp.source_path: imp.import_type for imp in imports}

        assert import_dict["./required_0x3333"] == "commonjs"
        assert import_dict["./dynamic_0x4444"] == "dynamic"

    def test_imports_sorted_by_line(self, sample_mixed_file):
        """Test that imports are sorted by line number."""
        imports = find_imports(str(sample_mixed_file))

        lines = [imp.line for imp in imports]
        assert lines == sorted(lines)

    def test_empty_file(self, temp_dir):
        """Test finding imports in an empty file."""
        empty_file = temp_dir / "empty.js"
        empty_file.write_text("")

        imports = find_imports(str(empty_file))
        assert imports == []

    def test_file_without_imports(self, temp_dir):
        """Test finding imports in a file with no imports."""
        no_imports = temp_dir / "no_imports.js"
        no_imports.write_text("console.log('hello');\nconst x = 1 + 2;")

        imports = find_imports(str(no_imports))
        assert imports == []


# =============================================================================
# Tests for generate_mapping_from_pattern
# =============================================================================

class TestGenerateMappingFromPattern:
    """Tests for the generate_mapping_from_pattern function."""

    def test_generates_mapping_for_obfuscated_imports(self):
        """Test that mapping is generated for obfuscated imports."""
        imports = [
            ImportMatch(1, 0, 1, 50, 'import x from "./mod_0x1234"', 'es6', './mod_0x1234'),
            ImportMatch(2, 0, 2, 50, 'import y from "./mod_0x5678"', 'es6', './mod_0x5678'),
        ]

        mapping = generate_mapping_from_pattern(imports)

        assert "./mod_0x1234" in mapping
        assert "./mod_0x5678" in mapping
        assert mapping["./mod_0x1234"] == "./module_1"
        assert mapping["./mod_0x5678"] == "./module_2"

    def test_skips_non_obfuscated_imports(self):
        """Test that non-obfuscated imports are skipped."""
        imports = [
            ImportMatch(1, 0, 1, 30, 'import React from "react"', 'es6', 'react'),
            ImportMatch(2, 0, 2, 50, 'import x from "./mod_0x1234"', 'es6', './mod_0x1234'),
        ]

        mapping = generate_mapping_from_pattern(imports)

        assert "react" not in mapping
        assert "./mod_0x1234" in mapping

    def test_custom_pattern(self):
        """Test with a custom obfuscation pattern."""
        imports = [
            ImportMatch(1, 0, 1, 50, 'import x from "./chunk_abc123"', 'es6', './chunk_abc123'),
            ImportMatch(2, 0, 2, 50, 'import y from "./mod_0x1234"', 'es6', './mod_0x1234'),
        ]

        mapping = generate_mapping_from_pattern(imports, pattern=r'chunk_[a-z0-9]+')

        assert "./chunk_abc123" in mapping
        assert "./mod_0x1234" not in mapping

    def test_preserves_directory_structure(self):
        """Test that directory structure is preserved in mapping."""
        imports = [
            ImportMatch(1, 0, 1, 50, 'import x from "./src/mod_0x1234"', 'es6', './src/mod_0x1234'),
        ]

        mapping = generate_mapping_from_pattern(imports)

        assert mapping["./src/mod_0x1234"] == "./src/module_1"

    def test_empty_imports_list(self):
        """Test with empty imports list."""
        mapping = generate_mapping_from_pattern([])
        assert mapping == {}


# =============================================================================
# Tests for load_mapping
# =============================================================================

class TestLoadMapping:
    """Tests for the load_mapping function."""

    def test_load_valid_mapping(self, sample_mapping_file):
        """Test loading a valid mapping file."""
        mapping = load_mapping(str(sample_mapping_file))

        assert mapping["./utils_0x1234"] == "./utils/helpers"
        assert mapping["./module_0xabcd"] == "./modules/main"

    def test_load_empty_mapping(self, temp_dir):
        """Test loading an empty mapping file."""
        empty_mapping = temp_dir / "empty_mapping.json"
        empty_mapping.write_text("{}")

        mapping = load_mapping(str(empty_mapping))
        assert mapping == {}

    def test_load_nonexistent_file_raises(self, temp_dir):
        """Test that loading nonexistent file raises an error."""
        with pytest.raises(FileNotFoundError):
            load_mapping(str(temp_dir / "nonexistent.json"))


# =============================================================================
# Tests for rewrite_file_with_mapping
# =============================================================================

class TestRewriteFileWithMapping:
    """Tests for the rewrite_file_with_mapping function."""

    def test_rewrites_double_quoted_imports(self, temp_dir):
        """Test rewriting imports with double quotes."""
        source = temp_dir / "source.js"
        source.write_text('import x from "./mod_0x1234";')

        mapping = {"./mod_0x1234": "./modules/real-name"}
        result = rewrite_file_with_mapping(str(source), mapping)

        assert 'import x from "./modules/real-name";' in result

    def test_rewrites_single_quoted_imports(self, temp_dir):
        """Test rewriting imports with single quotes."""
        source = temp_dir / "source.js"
        source.write_text("import x from './mod_0x1234';")

        mapping = {"./mod_0x1234": "./modules/real-name"}
        result = rewrite_file_with_mapping(str(source), mapping)

        assert "import x from './modules/real-name';" in result

    def test_rewrites_multiple_occurrences(self, temp_dir):
        """Test rewriting multiple occurrences of the same import."""
        source = temp_dir / "source.js"
        source.write_text('''
import x from "./mod_0x1234";
const y = require("./mod_0x1234");
''')

        mapping = {"./mod_0x1234": "./modules/real-name"}
        result = rewrite_file_with_mapping(str(source), mapping)

        assert result.count("./modules/real-name") == 2
        assert "./mod_0x1234" not in result

    def test_writes_to_output_file(self, temp_dir):
        """Test writing result to an output file."""
        source = temp_dir / "source.js"
        source.write_text('import x from "./mod_0x1234";')
        output = temp_dir / "output.js"

        mapping = {"./mod_0x1234": "./modules/real-name"}
        rewrite_file_with_mapping(str(source), mapping, str(output))

        assert output.exists()
        assert "./modules/real-name" in output.read_text()

    def test_preserves_unmatched_content(self, temp_dir):
        """Test that unmatched content is preserved."""
        source = temp_dir / "source.js"
        source.write_text('''
import React from "react";
import x from "./mod_0x1234";
console.log("hello");
''')

        mapping = {"./mod_0x1234": "./modules/real-name"}
        result = rewrite_file_with_mapping(str(source), mapping)

        assert 'import React from "react";' in result
        assert 'console.log("hello");' in result

    def test_no_changes_when_no_matches(self, temp_dir):
        """Test behavior when no mappings match."""
        source = temp_dir / "source.js"
        original_content = 'import React from "react";'
        source.write_text(original_content)

        mapping = {"./mod_0x1234": "./modules/real-name"}
        result = rewrite_file_with_mapping(str(source), mapping)

        assert result == original_content

    def test_escapes_special_regex_characters(self, temp_dir):
        """Test that special regex characters in paths are escaped."""
        source = temp_dir / "source.js"
        source.write_text('import x from "./mod[0].js";')

        mapping = {"./mod[0].js": "./modules/real-name.js"}
        result = rewrite_file_with_mapping(str(source), mapping)

        assert "./modules/real-name.js" in result


# =============================================================================
# Tests for ImportMatch dataclass
# =============================================================================

class TestImportMatch:
    """Tests for the ImportMatch dataclass."""

    def test_create_import_match(self):
        """Test creating an ImportMatch instance."""
        match = ImportMatch(
            line=1,
            column=0,
            end_line=1,
            end_column=30,
            text='import x from "module"',
            import_type="es6",
            source_path="module"
        )

        assert match.line == 1
        assert match.column == 0
        assert match.source_path == "module"
        assert match.import_type == "es6"

    def test_import_match_equality(self):
        """Test ImportMatch equality comparison."""
        match1 = ImportMatch(1, 0, 1, 30, 'text', 'es6', 'module')
        match2 = ImportMatch(1, 0, 1, 30, 'text', 'es6', 'module')

        assert match1 == match2


# =============================================================================
# Integration tests
# =============================================================================

class TestIntegration:
    """Integration tests combining multiple functions."""

    def test_full_workflow(self, temp_dir):
        """Test the full workflow: find imports, generate mapping, rewrite."""
        # Create source file
        source = temp_dir / "app.js"
        source.write_text('''
import auth from "./auth_0x1234";
import utils from "./utils_0x5678";
const api = require("./api_0x9abc");
''')

        # Find imports
        imports = find_imports(str(source))
        assert len(imports) >= 3

        # Generate mapping
        mapping = generate_mapping_from_pattern(imports)
        assert len(mapping) == 3

        # Rewrite
        result = rewrite_file_with_mapping(str(source), mapping)

        # Verify obfuscated names are replaced
        assert "./auth_0x1234" not in result
        assert "./utils_0x5678" not in result
        assert "./api_0x9abc" not in result

    def test_roundtrip_preserves_structure(self, temp_dir):
        """Test that rewriting preserves code structure."""
        original = '''// Comment
import x from "./mod_0x1234";

function test() {
  return x;
}

export default test;
'''
        source = temp_dir / "module.js"
        source.write_text(original)

        mapping = {"./mod_0x1234": "./real-module"}
        result = rewrite_file_with_mapping(str(source), mapping)

        # Structure preserved
        assert "// Comment" in result
        assert "function test()" in result
        assert "export default test;" in result

        # Only the import path changed
        assert "./real-module" in result
        assert "./mod_0x1234" not in result
