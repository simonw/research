# UV Run Flow Analysis - Investigation Notes

## Objective
Analyze what happens when a user runs `uv run myscript.py` in a folder with a pyproject.toml file containing dependencies.

## Investigation Log

### Setup
- Created investigation folder: uv-run-flow-analysis
- Starting analysis of uv codebase

- Deleted docs/, changelogs/, README.md, CHANGELOG.md, CONTRIBUTING.md, LICENSE files, and mkdocs configs

### Main `uv run` Entry Point
- Located in: `crates/uv/src/commands/project/run.rs`
- Main function: `run()` starting at line 80
- This is a ~2000 line file containing the complete flow

### High-Level Flow for `uv run myscript.py`:
1. **Recursion Depth Check** (line 117): Checks if `uv run` was recursively invoked too many times
2. **Requirements Validation** (line 131-150): Validates that requirements aren't coming from stdin or invalid sources
3. **Environment File Loading** (line 166-200): Loads `.env` file if specified
4. **Project Discovery** (line 541-583): 
   - Uses `VirtualProject::discover()` to find pyproject.toml
   - Checks for workspace configuration
5. **Python Interpreter Resolution** (line 643-680): Finds or downloads the correct Python version
6. **Locking Phase** (line 742-783): Creates/validates the lockfile
7. **Sync Phase** (line 835-871): Installs dependencies into the environment
8. **Command Execution** (line 1266-1318): Runs the actual script

### Key Dependencies to Investigate:
- `uv-workspace`: Project/workspace discovery and pyproject.toml parsing
- `uv-resolver`: Dependency resolution
- `project::lock`: Lockfile creation/validation
- `project::sync`: Environment synchronization and package installation


### Workspace Discovery (`uv-workspace` crate)
- Entry point: `Workspace::discover()` at line 196 in workspace.rs
- Searches upward from current directory to find pyproject.toml
- Checks for:
  - Explicit workspace root (`[tool.uv.workspace]`)
  - Project table (`[project]`)
  - Managed/unmanaged status (`tool.uv.managed`)
- Supports both explicit workspaces and implicit single-project workspaces
- Parses `PyProjectToml` struct which includes:
  - `project.dependencies`
  - `project.optional-dependencies`
  - `tool.uv.sources`
  - `tool.uv.workspace`
  - `dependency-groups` (PEP 735)

### Pyproject.toml Parsing
- Located in `crates/uv-workspace/src/pyproject.rs`
- Uses `toml_edit` crate for TOML parsing
- `PyProjectToml::from_string()` at line 125 parses raw TOML
- Main structure includes:
  - `project`: PEP 621 metadata (name, version, dependencies, etc.)
  - `tool`: Tool-specific config (tool.uv, etc.)
  - `dependency_groups`: Non-project dependency groups


### Lock and Sync Operations

#### Lock Operation (`crates/uv/src/commands/project/lock.rs`)
- Entry point: `lock()` function at line 82
- `LockOperation::new()` creates the lock operation
- Determines lock mode:
  - `Frozen`: Use existing lock, don't update
  - `Locked`: Validate lock is up-to-date
  - `Write`: Create/update the lockfile
- Lockfile stored in `uv.lock` at workspace root

#### Lockfile Format (`crates/uv-resolver/src/lock/mod.rs`)
- Current lockfile version: 1, revision: 3
- TOML-based format
- Stores resolved packages with:
  - Package name and version
  - Source (PyPI index, Git, local path, etc.)
  - Dependencies with environment markers
  - Hashes for integrity verification
  - Wheel metadata and compatibility tags

#### Sync Operation (`crates/uv/src/commands/project/sync.rs`)
- Entry point: `sync()` function at line 59
- Creates or discovers virtual environment
- Uses `do_sync()` to install dependencies
- Steps:
  1. Read existing site-packages
  2. Compare with lockfile requirements
  3. Download missing packages
  4. Install wheels into venv
  5. Validate installation

### Dependency Resolution (`uv-resolver` crate)
- Uses PubGrub algorithm for dependency resolution
- Key components:
  - `Resolver`: Main resolver entry point
  - `Manifest`: Collects requirements from pyproject.toml
  - `FlatIndex`: Indexes available package versions
  - `Lock`: Represents resolved dependency graph
- Handles:
  - Version constraints
  - Environment markers (OS, Python version, etc.)
  - Extras (optional dependencies)
  - Dependency groups (dev, test, etc.)
  - Universal resolution (cross-platform)

