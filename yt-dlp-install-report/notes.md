# yt-dlp[default] Installation Investigation

## Objective
Install yt-dlp[default] using pip and document what gets installed, including sizes and binary details.

## Investigation Steps

### 1. Initial Setup
- Created investigation folder: yt-dlp-install-report
- Starting investigation on: 2025-11-12

### 2. Installation Process
Ran: `pip install 'yt-dlp[default]'`

Packages installed:
- yt-dlp-2025.11.12 (main package)
- brotli-1.2.0 (compression library)
- mutagen-1.47.0 (audio metadata library)
- pycryptodomex-3.23.0 (cryptography library)
- websockets-15.0.1 (WebSocket protocol implementation)
- yt-dlp-ejs-0.3.1 (external JavaScript support)

Dependencies already installed:
- certifi (SSL certificates)
- requests (HTTP library)
- urllib3 (HTTP client)
- charset_normalizer (character encoding detection)
- idna (internationalized domain names)

### 3. Size Analysis
Total installation size: ~39.03 MB (40,920,799 bytes)

Package breakdown:
- yt_dlp: 22.29 MB (2246 files) - Largest package
- pycryptodomex: 8.84 MB (559 files) - Many cryptographic modules
- brotli: 4.95 MB (9 files) - Includes large .so binary
- mutagen: 1.49 MB (134 files)
- websockets: 1.30 MB (106 files)
- yt_dlp_ejs: 165.09 KB (15 files)

### 4. Binary Files
Found 44 binary (.so) files totaling 8.55 MB:
- Cryptodome: 42 binaries (3.59 MB) - Various cipher and hash implementations
- brotli: 1 binary (4.93 MB) - _brotli.cpython-311-x86_64-linux-gnu.so
- websockets: 1 binary (34.66 KB) - speedups module

The Brotli binary is the single largest binary file at 4.93 MB.

### 5. Executable
- Location: /usr/local/bin/yt-dlp
- Size: 205 bytes
- Type: Python script (wrapper that calls yt_dlp.main())
- Not a compiled binary, just a Python entry point

### 6. Key Findings
- The [default] extra includes compression (brotli), crypto (pycryptodomex), websockets, and metadata handling (mutagen)
- Most space is Python code in yt_dlp itself (22 MB)
- Binary libraries make up about 8.55 MB of the total
- pycryptodomex has the most binary files (42) for various crypto operations
- Installation includes shell completions (.fish) and man pages

