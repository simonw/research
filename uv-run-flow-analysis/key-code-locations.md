# Key Code Locations in UV Source

This document provides a quick reference to important code locations for understanding the `uv run` flow.

## Main Entry Points

### 1. Main Binary
- **File**: `crates/uv/src/bin/uv.rs`
- **Function**: `main()`
- **Purpose**: Entry point that calls `uv::main()`

### 2. CLI Command Routing
- **File**: `crates/uv/src/lib.rs`
- **Function**: `run(cli: Cli)`
- **Lines**: ~65-100
- **Purpose**: Routes CLI commands to appropriate handlers

### 3. Run Command Implementation
- **File**: `crates/uv/src/commands/project/run.rs`
- **Function**: `run()`
- **Lines**: 80-1319
- **Purpose**: Complete implementation of `uv run` command

## Project Discovery

### 4. Workspace Discovery
- **File**: `crates/uv-workspace/src/workspace.rs`
- **Function**: `Workspace::discover()`
- **Lines**: 196-293
- **Purpose**: Finds pyproject.toml and identifies workspace structure

### 5. Pyproject.toml Parsing
- **File**: `crates/uv-workspace/src/pyproject.rs`
- **Struct**: `PyProjectToml`
- **Lines**: 104-186
- **Function**: `from_string()`
- **Lines**: 125-131
- **Purpose**: Parses TOML into structured data

## Dependency Resolution

### 6. Lock Operation
- **File**: `crates/uv/src/commands/project/lock.rs`
- **Function**: `lock()`
- **Lines**: 82-221
- **Purpose**: Orchestrates the locking process

### 7. Resolver Main Logic
- **File**: `crates/uv-resolver/src/resolver.rs`
- **Struct**: `Resolver`
- **Purpose**: PubGrub-based dependency resolution

### 8. Lockfile Format
- **File**: `crates/uv-resolver/src/lock/mod.rs`
- **Struct**: `Lock`
- **Lines**: Throughout file
- **Constants**: VERSION (line 72), REVISION (line 75)
- **Purpose**: Lockfile data structure and serialization

## Environment Management

### 9. Virtual Environment Creation
- **File**: `crates/uv-virtualenv/src/lib.rs`
- **Function**: `create_venv()`
- **Purpose**: Creates Python virtual environments

### 10. Sync Operation
- **File**: `crates/uv/src/commands/project/sync.rs`
- **Function**: `sync()`
- **Lines**: 59-87
- **Function**: `do_sync()`
- **Purpose**: Synchronizes environment with lockfile

## Installation

### 11. Wheel Installation
- **File**: `crates/uv-install-wheel/src/lib.rs`
- **Function**: `install_wheel()`
- **Purpose**: Extracts and installs wheel files

### 12. Build Frontend
- **File**: `crates/uv-build-frontend/src/lib.rs`
- **Purpose**: PEP 517 build system interface

## Execution

### 13. Command Execution
- **File**: `crates/uv/src/commands/project/run.rs`
- **Enum**: `RunCommand`
- **Lines**: 1398-1646
- **Function**: `as_command()`
- **Lines**: 1459-1583
- **Purpose**: Converts run command into process

### 14. Process Management
- **File**: `crates/uv/src/child.rs`
- **Function**: `run_to_completion()`
- **Purpose**: Manages child process execution

## Supporting Infrastructure

### 15. Python Discovery
- **File**: `crates/uv-python/src/discovery.rs`
- **Purpose**: Finds and validates Python interpreters

### 16. HTTP Client
- **File**: `crates/uv-client/src/lib.rs`
- **Purpose**: HTTP client for package downloads

### 17. Cache Management
- **File**: `crates/uv-cache/src/lib.rs`
- **Purpose**: Download and build artifact caching

## Key Data Structures

### PyProjectToml
```rust
// Location: crates/uv-workspace/src/pyproject.rs:104
pub struct PyProjectToml {
    pub project: Option<Project>,
    pub tool: Option<Tool>,
    pub dependency_groups: Option<DependencyGroups>,
    pub raw: String,
}
```

### Project Metadata
```rust
// Location: crates/uv-workspace/src/pyproject.rs
pub struct Project {
    pub name: PackageName,
    pub version: Option<Version>,
    pub dependencies: Vec<Requirement>,
    pub optional_dependencies: BTreeMap<ExtraName, Vec<Requirement>>,
    pub requires_python: Option<VersionSpecifiers>,
    // ... additional fields
}
```

### Lock File
```rust
// Location: crates/uv-resolver/src/lock/mod.rs
pub struct Lock {
    version: LockVersion,
    fork_markers: Vec<UniversalMarker>,
    packages: Vec<Package>,
    manifest: ResolverManifest,
    // ... additional fields
}
```

### Package Entry
```rust
// Location: crates/uv-resolver/src/lock/mod.rs
pub struct Package {
    name: PackageName,
    version: Version,
    source: Source,
    dependencies: Vec<Dependency>,
    // ... additional fields
}
```

## Important Algorithms

### PubGrub Resolution
- **File**: `crates/uv-resolver/src/pubgrub/`
- **Description**: Version solving algorithm
- **Key concepts**:
  - Package terms (positive/negative)
  - Version ranges
  - Incompatibility tracking
  - Backtracking on conflicts

### Universal Resolution
- **File**: `crates/uv-resolver/src/universal_marker.rs`
- **Description**: Cross-platform dependency resolution
- **Key concepts**:
  - Environment markers (OS, Python version, etc.)
  - Marker simplification
  - Fork points for platform-specific deps

## Testing

### Integration Tests
- **Directory**: `crates/uv/tests/`
- **Description**: End-to-end tests for commands

### Unit Tests
- **Location**: Within each crate's source files
- **Convention**: `#[cfg(test)]` modules

## Configuration Files

### Cargo.toml (Workspace)
- **File**: `Cargo.toml` (root)
- **Purpose**: Rust workspace configuration

### Cargo.toml (UV Crate)
- **File**: `crates/uv/Cargo.toml`
- **Purpose**: Main uv binary dependencies and features

## Build System

### Binary Targets
- `uv`: Main CLI binary
- `uvw`: Windows GUI wrapper (optional)
- `uvx`: Legacy compatibility symlink

### Features
- `default`: Standard features (performance, static distribution)
- `self-update`: Self-update functionality
- `performance`: Memory allocator optimizations

## Development Workflow

1. **Build**: `cargo build --release`
2. **Test**: `cargo test`
3. **Run**: `cargo run -- <uv-command>`
4. **Debug**: `RUST_LOG=debug cargo run -- <uv-command>`

## Performance Considerations

### Concurrency
- Uses Tokio async runtime
- Parallel package downloads
- Concurrent dependency resolution

### Caching Layers
1. HTTP response cache
2. Downloaded package cache
3. Built wheel cache
4. Git repository cache
5. Interpreter discovery cache

### Optimization Techniques
- Pre-computed platform markers
- Lazy evaluation of requirements
- Incremental lockfile updates
- Hash-based change detection
