# OllamaChat - iOS Chat Client for Ollama

A native iOS app built with Swift and SwiftUI that provides a beautiful, intuitive interface for chatting with Ollama AI models running on local or remote servers.

## Features

- **Multiple Server Support**: Configure and manage multiple Ollama server instances with custom aliases
- **Persistent Storage**: All conversations, messages, and server configurations are stored in SQLite
- **Streaming Chat**: Real-time streaming responses from Ollama models
- **Conversation Management**: Create, view, and delete conversations organized by server
- **Model Selection**: Choose from available models on each server
- **Database Export**: Export your entire conversation history as a SQLite file via the iOS share sheet
- **Clean UI**: Modern SwiftUI interface with message bubbles, timestamps, and intuitive navigation

## Requirements

- **macOS**: 12.0 (Monterey) or later
- **Xcode**: 14.0 or later
- **iOS Deployment Target**: 15.0 or later
- **Active Ollama Server**: A running Ollama instance (local or remote)

## Installation & Compilation

### Step 1: Install Xcode

1. Download Xcode from the Mac App Store
2. Open Xcode and agree to the license terms
3. Install additional components when prompted

### Step 2: Create Xcode Project

1. Open Xcode
2. Select **File → New → Project**
3. Choose **iOS → App** template
4. Configure project settings:
   - **Product Name**: `OllamaChat`
   - **Team**: Select your Apple Developer team (or create a personal team)
   - **Organization Identifier**: `com.yourname` (use your own identifier)
   - **Interface**: SwiftUI
   - **Language**: Swift
   - **Storage**: None
   - **Include Tests**: Unchecked (optional)
5. Choose a location and click **Create**

### Step 3: Add Source Files

1. Delete the auto-generated `ContentView.swift` file from the project
2. Delete the auto-generated `OllamaChatApp.swift` file (we'll replace it)
3. Add all the Swift files from this repository to your Xcode project:
   - Right-click on the project navigator → **Add Files to "OllamaChat"**
   - Select all `.swift` files:
     - `OllamaChatApp.swift`
     - `Models.swift`
     - `DatabaseManager.swift`
     - `OllamaService.swift`
     - `ViewModels.swift`
     - `ServerListView.swift`
     - `ConversationListView.swift`
     - `ChatView.swift`
     - `DatabaseExportView.swift`
   - Ensure **"Copy items if needed"** is checked
   - Click **Add**

### Step 4: Configure Info.plist

1. In the project navigator, select the `Info.plist` file
2. Replace its contents with the `Info.plist` from this repository
3. **Important**: The Info.plist includes `NSAppTransportSecurity` settings that allow HTTP connections (needed for local Ollama servers)

### Step 5: Link SQLite Framework

The app uses SQLite3, which is included in iOS by default. Verify it's linked:

1. Select your project in the navigator
2. Select the **OllamaChat** target
3. Go to **Build Phases** tab
4. Expand **Link Binary With Libraries**
5. If `libsqlite3.tbd` is not listed, click **+** and add it

### Step 6: Configure Build Settings

1. Select your project → Target → **Build Settings**
2. Search for **"iOS Deployment Target"**
3. Set it to **iOS 15.0** or later
4. For simulator testing, select **Any iOS Simulator (arm64)** as the run destination
5. For device testing, connect your iPhone and select it as the run destination

### Step 7: Configure Signing

1. Select your project → Target → **Signing & Capabilities**
2. Enable **"Automatically manage signing"**
3. Select your Team (you can use a free Apple ID)
4. Xcode will automatically generate a provisioning profile

### Step 8: Build and Run

#### For iOS Simulator:
1. Select a simulator (e.g., "iPhone 15 Pro") from the scheme selector
2. Click the **Play** button or press **⌘R**
3. Wait for the build to complete
4. The app will launch in the simulator

#### For Physical Device:
1. Connect your iPhone via USB
2. Unlock your device and trust the computer if prompted
3. Select your device from the scheme selector
4. Click the **Play** button or press **⌘R**
5. **First time only**: On your iPhone, go to **Settings → General → VPN & Device Management → Developer App**, and trust the app
6. Return to the home screen and launch OllamaChat

## Setting Up Ollama Server

### Local Server (Mac)

1. Install Ollama on your Mac:
   ```bash
   brew install ollama
   ```

2. Start Ollama server:
   ```bash
   ollama serve
   ```

3. Pull a model (if you haven't already):
   ```bash
   ollama pull llama2
   ```

4. In the app, add server with URL: `http://localhost:11434`

### Remote Server

1. If running Ollama on another machine, use its IP address:
   - Example: `http://192.168.1.100:11434`

2. Make sure the Ollama server accepts connections from your network:
   ```bash
   OLLAMA_HOST=0.0.0.0:11434 ollama serve
   ```

## Usage Guide

### Adding a Server

1. Launch OllamaChat
2. Tap the **+** button in the top right
3. Enter your server URL (e.g., `http://localhost:11434`)
4. Optionally add an alias (e.g., "My Mac")
5. Tap **Test Connection** to verify it works
6. Tap **Save**

### Starting a Conversation

1. Tap on a server from the list
2. Tap the **+** button to create a new conversation
3. Enter a title (or leave blank for "New Chat")
4. Tap **Create**

### Chatting

1. Select a conversation
2. Choose a model from the dropdown at the top (if available)
3. Type your message in the input field
4. Tap the send button
5. Watch the AI response stream in real-time

### Exporting Database

1. Tap the **gear icon** on the server list
2. Select **Export Database**
3. Tap **Export Database**
4. Use the iOS share sheet to save or share the SQLite file

## Architecture

### Data Models
- **Server**: Stores Ollama server URL and alias
- **Conversation**: Groups messages by topic and server
- **Message**: Individual chat messages with role (user/assistant)

### Persistence Layer
- **DatabaseManager**: SQLite wrapper using native SQLite3 C API
- **Database Schema**: Three tables (servers, conversations, messages) with foreign key relationships

### API Service
- **OllamaService**: Handles HTTP communication with Ollama API
- **Streaming Support**: Parses newline-delimited JSON responses in real-time

### Views
- **ServerListView**: Manage multiple Ollama servers
- **ConversationListView**: Browse conversations for a server
- **ChatView**: Main chat interface with message bubbles
- **DatabaseExportView**: Export functionality with share sheet

## Troubleshooting

### Build Errors

**Error: "No such module 'SQLite3'"**
- Solution: Add `libsqlite3.tbd` in Build Phases → Link Binary With Libraries

**Error: "Sandbox: rsync.samba not permitted"**
- Solution: This is a warning, not an error. The app will still build and run.

**Error: "Failed to register bundle identifier"**
- Solution: Change the Bundle Identifier in project settings to something unique

### Runtime Issues

**Cannot connect to localhost**
- When using iOS Simulator: Use `http://localhost:11434`
- When using physical device: Use your Mac's IP address (e.g., `http://192.168.1.x:11434`)
- Verify Ollama is running: `curl http://localhost:11434/api/tags`

**"Connection failed" when testing server**
- Check that Ollama server is running
- Verify the URL is correct
- Ensure no firewall is blocking the connection
- For remote connections, make sure Ollama accepts external connections

**No models available**
- Pull a model in Ollama: `ollama pull llama2`
- Restart the Ollama server
- Refresh the app by selecting a different server and back

**App crashes on launch**
- Check Xcode console for specific error
- Clean build folder: Product → Clean Build Folder
- Delete derived data: Xcode → Preferences → Locations → Derived Data → Delete

## Project Structure

```
OllamaChat/
├── OllamaChatApp.swift          # Main app entry point
├── Models.swift                  # Data models
├── DatabaseManager.swift         # SQLite persistence layer
├── OllamaService.swift          # API service for Ollama
├── ViewModels.swift             # State management
├── ServerListView.swift         # Server management UI
├── ConversationListView.swift   # Conversation list UI
├── ChatView.swift               # Chat interface
├── DatabaseExportView.swift     # Database export UI
└── Info.plist                   # App configuration
```

## Technical Details

- **SwiftUI**: Declarative UI framework
- **Combine**: Reactive state management via @Published properties
- **SQLite3**: Native C API for database operations
- **URLSession**: Streaming HTTP requests
- **MVVM Pattern**: Separation of views, view models, and models

## Future Enhancements

Potential improvements for future versions:

- [ ] Image support for multimodal models
- [ ] Conversation search functionality
- [ ] Message editing and regeneration
- [ ] Export individual conversations
- [ ] Dark mode customization
- [ ] Custom system prompts
- [ ] Temperature and other model parameters
- [ ] Markdown rendering in messages
- [ ] Code syntax highlighting
- [ ] iPad optimization with split view

## License

This project is provided as-is for educational and personal use.

## Acknowledgments

- Built for [Ollama](https://ollama.ai/) - Run AI models locally
- Uses SwiftUI for native iOS experience
- SQLite for reliable data persistence
