# yt-dlp[default] Installation Analysis

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

## Overview

This report documents a detailed analysis of installing `yt-dlp[default]` via pip, including all packages installed, file sizes, and binary dependencies.

**Date:** 2025-11-12
**Python Version:** 3.11
**Platform:** Linux x86_64
**yt-dlp Version:** 2025.11.12

## Executive Summary

Installing `yt-dlp[default]` results in:
- **6 new packages** installed
- **3,069 total files**
- **~39.03 MB** total disk space
- **44 binary (.so) files** totaling 8.55 MB
- **1 executable** (Python wrapper script)

## Installation Command

```bash
pip install 'yt-dlp[default]'
```

## Packages Installed

### New Packages (6)

| Package | Version | Size | Files | Description |
|---------|---------|------|-------|-------------|
| **yt-dlp** | 2025.11.12 | 22.29 MB | 2,246 | Feature-rich command-line audio/video downloader |
| **pycryptodomex** | 3.23.0 | 8.84 MB | 559 | Cryptographic library for Python |
| **brotli** | 1.2.0 | 4.95 MB | 9 | Python bindings for Brotli compression library |
| **mutagen** | 1.47.0 | 1.49 MB | 134 | Audio metadata reading/writing library |
| **websockets** | 15.0.1 | 1.30 MB | 106 | WebSocket Protocol implementation (RFC 6455 & 7692) |
| **yt-dlp-ejs** | 0.3.1 | 165.09 KB | 15 | External JavaScript support for yt-dlp |
| **TOTAL** | | **39.03 MB** | **3,069** | |

### Already Satisfied Dependencies

These packages were already installed in the environment:
- **certifi** (2025.10.5) - SSL certificate bundle
- **requests** (2.32.5) - HTTP library for Python
- **urllib3** (2.5.0) - HTTP client
- **charset_normalizer** (3.4.4) - Character encoding detection
- **idna** (3.11) - Internationalized Domain Names in Applications

## Binary Files Analysis

The installation includes **44 compiled binary files** (.so shared objects) totaling **8.55 MB**:

### Binary Distribution by Package

| Package | Binary Files | Total Binary Size | Details |
|---------|--------------|-------------------|---------|
| **Cryptodome** | 42 | 3.59 MB | Cipher and hash implementations |
| **brotli** | 1 | 4.93 MB | Single large compression binary |
| **websockets** | 1 | 34.66 KB | WebSocket speedups module |
| **TOTAL** | **44** | **8.55 MB** | |

### Notable Binary Files

#### Largest Binary: Brotli Compression Module
- **File:** `_brotli.cpython-311-x86_64-linux-gnu.so`
- **Size:** 4.93 MB
- **Type:** ELF 64-bit LSB shared object, x86-64
- **Purpose:** Native compression/decompression for Brotli algorithm
- **Dependencies:** Standard C libraries (libc, libpthread, libm, libgcc_s)

#### Cryptodome Binaries (42 files, 3.59 MB)
The pycryptodomex package includes numerous compiled cryptographic modules:
- **Cipher implementations:** AES, DES, 3DES, ARC2, ARC4, Blowfish, CAST, ChaCha20, Salsa20
- **Hash functions:** SHA256, SHA512, SHA1, SHA224, SHA384, MD2, MD4, MD5, BLAKE2s, BLAKE2b, keccak, RIPEMD160, Poly1305
- **Utilities:** PKCS#1 decode, scrypt, various cipher modes (CBC, CFB, CTR, ECB, OCB, OFB, GCM)
- **Size range:** 20 KB to 283 KB per binary
- **Architecture:** ELF 64-bit, using Python's stable ABI (abi3)

#### WebSocket Binary
- **File:** `websockets/speedups.cpython-311-x86_64-linux-gnu.so`
- **Size:** 34.66 KB
- **Purpose:** Performance optimizations for WebSocket protocol operations

### Binary Characteristics

All binaries share these characteristics:
- **Format:** ELF 64-bit LSB shared object
- **Architecture:** x86-64
- **Linking:** Dynamically linked
- **Dependencies:** Standard Linux libraries (libc.so.6, libpthread.so.0, ld-linux-x86-64.so.2)
- **Debug info:** Present, not stripped (suitable for debugging)

## Executable

### yt-dlp Command
- **Location:** `/usr/local/bin/yt-dlp`
- **Size:** 205 bytes
- **Type:** Python script (ASCII text)
- **Not a binary:** It's a simple Python wrapper script

**Content:**
```python
#!/usr/bin/python3
# -*- coding: utf-8 -*-
import re
import sys
from yt_dlp import main
if __name__ == '__main__':
    sys.argv[0] = re.sub(r'(-script\.pyw|\.exe)?$', '', sys.argv[0])
    sys.exit(main())
```

The executable is just an entry point that imports and calls `yt_dlp.main()`.

## File Type Distribution

### yt-dlp (2,246 files)
- **Python files:** 1,115 (.py)
- **Bytecode:** 1,115 (.pyc)
- **JavaScript:** 3 (.js)
- **Documentation:** Man page (.1), shell completion (.fish)
- **Metadata:** 9 files
- **License/README:** 2 (.txt)

### pycryptodomex (559 files)
- **Python files:** 206 (.py)
- **Type stubs:** 97 (.pyi) - for type checking
- **Binaries:** 42 (.so)
- **Bytecode:** 206 (.pyc)
- **Documentation:** 2 (.rst)
- **Metadata:** 6 files

### mutagen (134 files)
- **Python files:** 57 (.py)
- **Bytecode:** 57 (.pyc)
- **Man pages:** 6 (.1)
- **Metadata:** 11 files
- **License:** 2 (.txt)
- **Type marker:** 1 (.typed)

### websockets (106 files)
- **Python files:** 47 (.py)
- **Bytecode:** 47 (.pyc)
- **Binary:** 1 (.so)
- **Type stubs:** 1 (.pyi)
- **C source:** 1 (.c) - included in distribution
- **Metadata:** 6 files
- **License:** 2 (.txt)
- **Type marker:** 1 (.typed)

### brotli (9 files)
- **Binary:** 1 (.so) - 4.93 MB
- **Bytecode:** 1 (.pyc)
- **Python:** 1 (.py)
- **License:** 1 (.txt)
- **Metadata:** 5 files

### yt-dlp-ejs (15 files)
- **Python files:** 4 (.py)
- **Bytecode:** 4 (.pyc)
- **JavaScript:** 2 (.js)
- **Metadata:** 5 files

## What is the [default] Extra?

The `[default]` extra for yt-dlp includes optional dependencies that enhance functionality:

1. **brotli** - Enables Brotli decompression for downloaded content
2. **pycryptodomex** - Provides cryptographic capabilities for encrypted streams
3. **websockets** - Enables WebSocket support for live streams and certain services
4. **mutagen** - Allows reading/writing audio file metadata and tags
5. **yt-dlp-ejs** - Enables execution of JavaScript code for certain extractors

Without these, yt-dlp can still function for basic downloads but may have limited capabilities with encrypted content, live streams, and metadata handling.

## Disk Space Breakdown

### By Size
1. yt-dlp main package: 22.29 MB (57.1%)
2. pycryptodomex: 8.84 MB (22.7%)
3. brotli: 4.95 MB (12.7%)
4. mutagen: 1.49 MB (3.8%)
5. websockets: 1.30 MB (3.3%)
6. yt-dlp-ejs: 0.17 MB (0.4%)

### By Type
- **Python source code:** ~50% of total size
- **Binary libraries:** ~22% of total size (8.55 MB)
- **Bytecode (.pyc):** ~25% of total size
- **Documentation/metadata:** ~3% of total size

## System Requirements

### Runtime Dependencies (Shared Libraries)
All binaries depend on:
- `libc.so.6` - GNU C Library
- `libpthread.so.0` - POSIX threads library
- `ld-linux-x86-64.so.2` - Dynamic linker/loader

Some binaries also use:
- `libm.so.6` - Math library
- `libgcc_s.so.1` - GCC runtime library

These are standard libraries available on all modern Linux systems.

## Security Considerations

### Binary Integrity
- All binaries include debug information (not stripped)
- BuildID present for verification
- Standard system library dependencies only
- No unusual or suspicious dependencies

### Cryptographic Components
The pycryptodomex package provides:
- Modern ciphers: AES, ChaCha20
- Legacy ciphers: DES, 3DES, ARC4, Blowfish (for compatibility)
- Hash functions: SHA-2 family, SHA-3 (Keccak), BLAKE2
- Used for decrypting protected video streams

## Conclusion

Installing `yt-dlp[default]` is a moderate-sized installation at ~39 MB, with the majority of space consumed by:

1. The yt-dlp Python codebase itself (1,100+ Python modules for various video extractors)
2. Cryptographic binaries for handling encrypted content
3. Compression libraries for efficient data handling

The installation includes 44 compiled binaries that provide performance-critical operations (compression, cryptography, WebSocket handling) while the main functionality is implemented in Python for maintainability and extensibility.

The [default] extra is recommended for full functionality, especially when downloading from sites that use:
- Brotli compression
- Encrypted streams (HLS, DASH)
- WebSocket-based live streams
- When metadata manipulation is needed

## Files Included

This investigation includes:
- `notes.md` - Development notes and findings
- `README.md` - This comprehensive report
- `install_output.txt` - Raw pip installation output
- `package_details.txt` - Detailed package information
- `package_analysis.txt` - File-by-file size analysis
- `binary_details.txt` - Complete binary analysis with dependencies
- `analyze_packages.py` - Python script for package analysis
- `analyze_binaries.py` - Python script for binary analysis
