# JSLinux Notebook Investigation Notes

## Starting Investigation
- Created folder: jslinux-notebook
- Task: Download and test jslinux-notebook.html with Playwright

## Steps Taken

### 1. Initial Setup
- Downloaded HTML file from https://static.simonwillison.net/static/2025/jslinux-notebook.html (8964 bytes)
- File is a self-contained notebook interface for JSLinux
- It embeds an iframe to https://bellard.org/jslinux/vm.html with Alpine Linux
- Has a fallback "demo mode" with simulated commands if iframe access fails due to CORS
- Features: notebook-style cells, run buttons, keyboard shortcuts (Ctrl+Enter)

### 2. Understanding the Application
- Loads JSLinux in an iframe
- Tries to access the terminal from iframe.contentWindow.term
- Falls back to demo mode if cross-origin access is blocked
- Demo mode simulates common commands: ls, pwd, whoami, date, etc.

### 3. User Requirement Change
- User wants a COMPLETELY self-hosted version - no iframe to external sites
- Need to download JSLinux files locally
- Create a setup script to fetch dependencies
- Use .gitignore to exclude fetched files from commits
- Only commit our modifications and the setup script

### 4. Downloaded JSLinux Package
- Downloaded jslinux-2019-12-21.tar.gz from bellard.org/tinyemu (13.3MB)
- Package contains:
  - jslinux.js - main JavaScript loader
  - term.js - terminal emulator
  - x86emu.js / x86emu-wasm.js - x86 emulator
  - riscvemu64.js / riscvemu64-wasm.js - RISC-V 64-bit emulator
  - kernel files and root filesystems
  - Configuration files (.cfg)

### 5. How JSLinux Works
- Uses Term.js for terminal display
- Loads WebAssembly/JS emulator based on CPU type
- Accesses global `term` object to send/receive data
- Start function: `start_vm(user, pwd)`
- Terminal handler: `term_handler(str)` sends input to emulator
- Console output: `console_write1(charCode)` writes to terminal

### 6. Created Files
- setup.sh: Bash script to download and extract JSLinux
- .gitignore: Excludes downloaded files (jslinux/, *.tar.gz, etc.)
- notebook.html: New notebook interface that integrates with local JSLinux
  - Hides the terminal UI but uses it for execution
  - Provides notebook-style cells for commands
  - Captures terminal output via overriding term.write()
  - Keyboard shortcuts: Ctrl+Enter to run cells
- test_notebook.py: Playwright test script

### 7. Challenges
- Playwright dependencies issue in Docker environment
- JSLinux takes a few seconds to boot (3-5 seconds)
- Capturing terminal output requires careful timing
- Terminal uses ANSI codes that need to be stripped
