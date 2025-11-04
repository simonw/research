# Datasette App Tauri POC - Development Notes

## Objective
Create a proof-of-concept Tauri version of datasette-app, which is currently an Electron application that bundles Python to run Datasette.

## Starting Investigation
- Created project folder: datasette-app-tauri
- Task: Replace Electron with Tauri while maintaining the same functionality

## Analysis of datasette-app (Electron Version)

### Key Architecture Components:
1. **Electron Framework**: Uses Node.js for the backend/main process
2. **Python Bundling**: Includes standalone Python 3.9 in app resources
3. **Datasette Process**: Spawns Datasette server as child process
4. **Communication**: HTTP API between Electron and Datasette
5. **Virtual Environment**: Creates venv at ~/.datasette-app/venv
6. **Key Features**:
   - Opens SQLite databases and CSV files
   - Plugin installation via pip
   - Access control (localhost vs network)
   - Menu system for navigation
   - IPC for plugin management

### Main Components to Port:
- `main.js` (1180 lines) - Main Electron process
  - DatasetteServer class: manages Python process
  - Menu building and window management
  - File opening and CSV import
  - Plugin installation/uninstallation
- `preload.js` - Electron preload script
- `loading.html`, `progress.html`, etc. - HTML pages for UI states
- `package.json` - Build configuration

### Challenges for Tauri Port:
1. Replace Node.js backend with Rust
2. Replace Electron IPC with Tauri commands
3. Maintain Python process management
4. Port menu system to Tauri
5. Handle file dialogs and system integration

## Tauri Architecture Study

### Key Differences from Electron:
1. **Backend Language**: Rust instead of Node.js
2. **IPC**: Uses `#[tauri::command]` for frontend-callable functions
3. **Configuration**: `tauri.conf.json` for app settings
4. **Process Management**: Use Rust's `std::process::Command`
5. **File System**: Use Rust's `std::fs` for file operations
6. **Menu System**: Tauri has its own menu API
7. **Size**: Much smaller bundle size (Rust vs. Chromium+Node)

### Design for Tauri Port:

#### Project Structure:
```
datasette-app-tauri/
├── src-tauri/
│   ├── src/
│   │   └── main.rs      # Main Rust code
│   ├── Cargo.toml        # Rust dependencies
│   └── tauri.conf.json   # Tauri configuration
├── src/
│   ├── index.html        # Main UI
│   └── loading.html      # Loading screen
└── README.md
```

#### Key Components to Implement:
1. **Datasette Process Manager** (Rust)
   - Spawn Python/Datasette process
   - Manage lifecycle
   - Handle stdout/stderr

2. **Tauri Commands** (Rust)
   - `start_datasette()` - Start the server
   - `open_file(path)` - Open database/CSV
   - `get_server_url()` - Get localhost URL

3. **UI** (HTML/JavaScript)
   - Loading screen
   - Main webview showing Datasette
   - Use Tauri's invoke API to call Rust commands

## Implementation Phase Started

### Files Created:

1. **src-tauri/Cargo.toml** - Rust dependencies
   - Added tauri 2.x with necessary features
   - Added plugins for shell and dialog
   - Added tokio for async runtime

2. **src-tauri/build.rs** - Tauri build script

3. **src-tauri/tauri.conf.json** - Tauri configuration
   - Product name: Datasette
   - Bundle identifier: io.datasette.app
   - CSP configured to allow localhost connections
   - Shell and dialog plugins enabled

4. **src-tauri/src/main.rs** - Main entry point
   - Simple wrapper that calls the lib

5. **src-tauri/src/lib.rs** - Core application logic (200+ lines)
   - `DatasetteState` struct to manage process and config
   - `start_datasette()` command - starts Datasette server
   - `get_server_url()` command - returns server URL
   - `stop_datasette()` command - stops the server
   - `open_database_file()` command - opens a database file
   - `check_datasette_available()` command - checks if datasette is installed
   - Proper cleanup on app exit

6. **src/loading.html** - Loading screen UI
   - Beautiful gradient design
   - Auto-starts Datasette on load
   - Error handling and retry functionality
   - "Open Database" button for file selection
   - Uses Tauri's invoke API to call Rust commands

7. **src/index.html** - Simple redirect to loading.html

### Implementation Notes:

**Key Differences from Electron Version:**
- Used Rust instead of Node.js for backend
- Used Tauri commands instead of Electron IPC
- Simplified Python management (uses system Python for POC)
- No menu system yet (could be added with Tauri's menu API)
- More lightweight and faster

**Simplifications for POC:**
- Uses system Python/Datasette instead of bundling
- Fixed port (8765) instead of dynamic port finding
- No plugin management yet
- No venv creation (relies on system installation)
- No menu bar integration

**Next Steps:**
- Build the application (IN PROGRESS)
- Test it
- Document findings

## Build Process

### Prerequisites Installed:
- Rust/Cargo: 1.90.0 ✓
- Node.js: v22.21.0 ✓
- npm: 10.9.4 ✓
- Datasette: 0.65.1 ✓

### Building:
Running `cargo build` in src-tauri directory...
This compiles the Rust backend and creates the Tauri application.

**Build Issues Encountered:**
1. Missing GTK dependencies - Fixed by installing libgtk-3-dev, libwebkit2gtk-4.1-dev, etc.
2. Missing icon files - Fixed by copying placeholder icons from Tauri examples
3. Tauri feature mismatch - Removed `protocol-asset` feature from Cargo.toml
4. Missing Clone trait on DatasetteConfig - Added Clone derive
5. API changes in Tauri 2.x - Removed deprecated `eval()` and `listen()` calls

**Build Success!**
- Build completed in 16.05s
- Binary created: `target/debug/datasette-app-tauri` (183MB)
- Library files also generated

### Test Data:
Created test SQLite database at /tmp/test.db with sample books table for testing.

## Build vs Electron Comparison

### Bundle Size:
- **Tauri Debug Build**: 183MB executable
- **Electron (datasette-app)**: ~120MB app bundle (but includes Chromium)
- **Tauri Release Build**: Would be significantly smaller (estimated ~40-60MB)

### Key Differences:
1. **Backend**: Rust vs Node.js
2. **Build Time**: Slower initial build for Rust (compiling 500+ crates), but faster incremental builds
3. **Runtime**: Uses system WebView (WebKit on Linux) vs bundled Chromium
4. **Memory**: Tauri typically uses less memory (doesn't bundle browser)
5. **Dependencies**: Requires GTK/WebKit dev libraries on Linux
