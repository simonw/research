# JSLinux Notebook

A notebook-style interface for running Linux commands through JSLinux - a complete x86 and RISC-V emulator running entirely in your browser.

## Overview

This project provides a Jupyter-like notebook interface for JSLinux, allowing you to:
- Run Linux commands in isolated cells
- Save and reload your work sessions
- Export notebooks as HTML
- Use command shortcuts for common operations
- Manage cells (add, delete, reorder)

## Features

### Two Versions Available

#### 1. `notebook.html` - Basic Version
- Simple cell-based interface
- Run Linux commands in cells
- Multiple cells with individual outputs
- Keyboard shortcuts (Ctrl+Enter to run)
- Auto-clear and cell management

#### 2. `notebook-enhanced.html` - Full-Featured Version
Includes everything from the basic version plus:

**Persistence**
- ✅ Auto-save to browser localStorage
- ✅ Auto-load previous session on startup
- ✅ Save notebook to JSON file
- ✅ Load notebook from JSON file

**Cell Management**
- ✅ Delete cells with confirmation
- ✅ Move cells up/down to reorder
- ✅ Execution counter (Cell [1], Cell [2], etc.)
- ✅ Cell action buttons in headers

**Output Management**
- ✅ Collapsible outputs for long results
- ✅ Clear individual or all outputs
- ✅ Output persistence with notebook state

**Productivity**
- ✅ Command shortcuts panel (ls, df, free, ps, etc.)
- ✅ Quick-insert common commands
- ✅ Toggle shortcuts panel
- ✅ Toast notifications for feedback

**Export**
- ✅ Export as standalone HTML
- ✅ Formatted output with timestamps
- ✅ Shareable exported notebooks

## Setup

### Prerequisites
- Python 3.x (for HTTP server)
- Modern web browser with WebAssembly support

### Installation

1. **Clone or download this repository**

2. **Run the setup script to download JSLinux files:**
   ```bash
   cd jslinux-notebook
   ./setup.sh
   ```

   This will:
   - Download jslinux-2019-12-21.tar.gz from bellard.org (~13MB)
   - Extract all necessary files to the `jslinux/` directory
   - Set up the complete JSLinux environment

3. **Start a local web server:**
   ```bash
   python -m http.server 8888
   ```

4. **Open in your browser:**
   - Basic version: http://localhost:8888/notebook.html
   - Enhanced version: http://localhost:8888/notebook-enhanced.html

## Usage

### Running Commands

1. Type a Linux command in a cell (e.g., `ls`, `uname -a`, `cat /etc/issue`)
2. Click "Run" or press **Ctrl+Enter** (Cmd+Enter on Mac)
3. Wait a moment for the command to execute
4. View the output below the cell

### Common Commands to Try

```bash
ls -la              # List files with details
uname -a            # Show system information
cat /etc/issue      # Display Linux distribution
free -m             # Show memory usage
df -h               # Show disk space
ps aux              # List running processes
cat /proc/cpuinfo   # Display CPU information
pwd                 # Print working directory
whoami              # Show current user
date                # Display current date/time
```

### Managing Cells

**Add a new cell:**
- Click the "+ Add command cell" button at the bottom

**Delete a cell:**
- Click the "Delete" button in the cell header (enhanced version)

**Reorder cells:**
- Use ↑ and ↓ buttons in cell headers (enhanced version)

### Saving Your Work

**Enhanced version features:**

- **Auto-save**: Your notebook is automatically saved to browser localStorage as you work
- **Manual save**: Click "💾 Save" to download as JSON file
- **Load**: Click "📂 Load" to import a saved JSON file
- **Export**: Click "📄 Export" to download as standalone HTML

### Command Shortcuts (Enhanced Version)

Click "📋 Shortcuts" to show/hide the quick command panel with buttons for:
- `ls -la` - Detailed directory listing
- `cat /etc/issue` - Show OS version
- `uname -a` - System information
- `free -m` - Memory usage
- `df -h` - Disk space
- `ps aux` - Process list
- `cat /proc/cpuinfo` - CPU details
- `pwd` - Current directory

## Architecture

### Self-Hosted Design

This project is completely self-hosted - no external dependencies after setup:

1. **Setup script** (`setup.sh`) downloads JSLinux once
2. **Local files** are served from your machine
3. **No external iframes** or API calls during use
4. **Complete offline capability** after initial download

### Components

- `notebook.html` / `notebook-enhanced.html` - Notebook interfaces
- `jslinux/` - JSLinux emulator files (downloaded by setup.sh)
  - `jslinux.js` - Main JSLinux loader
  - `term.js` - Terminal emulator
  - `x86emu.js` / `x86emu-wasm.js` - x86 emulator
  - `riscvemu64.js` / `riscvemu64-wasm.js` - RISC-V emulator
  - Kernel binaries and root filesystems
- `setup.sh` - Download and extraction script
- `.gitignore` - Excludes large binary files from git

### How It Works

1. JSLinux loads and boots a real Linux kernel in WebAssembly
2. A terminal emulator (term.js) provides the interface
3. The notebook intercepts terminal output via JavaScript
4. Commands are sent to the emulator and output is captured
5. ANSI color codes are stripped for clean display

## Development

### Testing

A Playwright test suite is included:

```bash
pip install playwright
playwright install chromium
python test_notebook.py
```

Note: Some environments may have missing system dependencies for Playwright.

### File Structure

```
jslinux-notebook/
├── README.md                      # This file
├── setup.sh                       # Setup script
├── notebook.html                  # Basic notebook
├── notebook-enhanced.html         # Enhanced notebook
├── test_notebook.py              # Playwright tests
├── notes.md                       # Development notes
├── .gitignore                     # Git ignore rules
└── jslinux/                       # Downloaded JSLinux files (not in git)
    └── jslinux-2019-12-21/
        ├── jslinux.js
        ├── term.js
        ├── x86emu-wasm.js
        └── ... (other files)
```

## Technical Details

### Boot Time
- JSLinux takes 3-5 seconds to boot after page load
- "Linux ready" status appears when system is ready
- Run buttons are disabled until boot completes

### Output Capture
- Terminal output is captured by overriding `term.write()`
- ANSI escape codes are stripped for clean display
- Timing delays are used to collect complete output

### Storage
- localStorage is used for auto-save (enhanced version)
- Maximum ~5-10MB storage depending on browser
- Cleared when browser data is cleared

### Browser Compatibility
- Requires WebAssembly support
- Tested on Chrome, Firefox, Safari
- Best performance on Chrome/Edge

## Credits

- **JSLinux** by Fabrice Bellard - https://bellard.org/jslinux/
- **TinyEMU** - The underlying emulator system
- Original JSLinux is MIT licensed

## Limitations

- Terminal output capture relies on timing (may miss fast commands)
- No true command history navigation (up/down arrows)
- Some complex interactive commands may not work properly
- File system changes are not persistent between page reloads
- Limited to BusyBox utilities included in the Alpine Linux image

## Future Improvements

Potential enhancements:
- Dark mode toggle
- Syntax highlighting for commands
- Command history panel
- Multiple kernel/distro options
- Persistent filesystem using browser storage
- Split-screen view (notebook + terminal)
- Markdown cells for documentation
- Cell execution timing
- Download output as text file

## Troubleshooting

**"Terminal not ready yet!" error:**
- Wait a few more seconds for JSLinux to boot
- Refresh the page if it takes longer than 10 seconds

**Commands not producing output:**
- Some commands may be too fast to capture
- Try adding `; sleep 0.1` after command
- Check browser console for errors

**Auto-save not working:**
- Check if browser has localStorage enabled
- Clear browser cache if quota is exceeded
- Try using "Save" button to export manually

**Page not loading:**
- Ensure HTTP server is running
- Check that JSLinux files were downloaded (run `./setup.sh`)
- Verify you're accessing http://localhost:8888 (not file://)

## License

This notebook interface is provided as-is for educational purposes. JSLinux itself is MIT licensed by Fabrice Bellard.
