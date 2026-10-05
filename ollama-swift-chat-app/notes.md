# Ollama Swift Chat App - Development Notes

## Project Goal
Build a complete iOS chat client for Ollama servers using Swift and SwiftUI.

## Requirements
- Multiple Ollama server URLs with optional aliases
- Persistent server configurations
- Streaming chat UI
- Start new conversations and continue existing ones
- Persist all conversations in SQLite
- Export SQLite database via system share panel
- Detailed compilation instructions

## Architecture Design

### Data Models
1. **Server**: id, url, alias, createdAt
2. **Conversation**: id, serverId, title, createdAt, updatedAt
3. **Message**: id, conversationId, role (user/assistant), content, timestamp

### Core Components
1. **Persistence Layer**: SQLite wrapper using GRDB or native SQLite
2. **API Service**: OllamaService for streaming chat API
3. **ViewModels**: ServersViewModel, ConversationsViewModel, ChatViewModel
4. **Views**: ServerListView, ConversationListView, ChatView

### Tech Stack
- SwiftUI for UI
- SQLite for persistence (using GRDB.swift for easier Swift integration)
- URLSession for streaming API calls
- Combine for reactive state management

## Development Progress

### Phase 1: Setup and Models ✓
- Created project folder structure
- Defined data models for Server, Conversation, Message
- Created Ollama API request/response models

### Phase 2: Core Infrastructure ✓
- Implemented DatabaseManager with SQLite3 C API
  - Tables: servers, conversations, messages
  - Full CRUD operations for all entities
  - Foreign key constraints for data integrity
- Built OllamaService for API communication
  - Streaming chat support with newline-delimited JSON parsing
  - Model listing endpoint
  - Connection testing

### Phase 3: State Management ✓
- Created ViewModels using @Published properties and Combine
  - ServersViewModel: Server management
  - ConversationsViewModel: Conversation management
  - ChatViewModel: Chat state and streaming message handling

### Phase 4: User Interface ✓
- ServerListView: Add, list, and delete servers
- AddServerView: Server configuration with connection testing
- ConversationListView: Browse and manage conversations per server
- ChatView: Main chat interface with streaming message display
  - Message bubbles with role-based styling
  - Model selector with picker
  - Real-time streaming updates
- DatabaseExportView: Share SQLite database via iOS share sheet
- SettingsView: App info and database export access

### Phase 5: Polish ✓
- Info.plist configuration for HTTP connections (NSAppTransportSecurity)
- Main app entry point with WindowGroup scene
- Comprehensive README with step-by-step compilation instructions

## Key Technical Decisions

1. **Native SQLite3 over third-party libraries**:
   - Pros: No external dependencies, smaller binary, direct control
   - Cons: More verbose code, manual memory management

2. **Streaming chat implementation**:
   - URLSession dataTask with newline-delimited JSON parsing
   - Real-time UI updates via @Published properties
   - Temporary message approach for streaming content

3. **MVVM architecture**:
   - Clear separation of concerns
   - SwiftUI @StateObject and @ObservedObject for reactive updates
   - ViewModels handle business logic and data operations

4. **UUID-based primary keys**:
   - Better for distributed systems
   - No auto-increment issues
   - Easy to handle in Swift with UUID type

## Challenges and Solutions

### Challenge 1: Streaming Chat Implementation
- **Problem**: Ollama returns newline-delimited JSON, need to parse incrementally
- **Solution**: Parse full response string split by newlines, decode each JSON line

### Challenge 2: Real-time UI Updates During Streaming
- **Problem**: Need to update message content as chunks arrive
- **Solution**: Create temporary message with unique ID, update in-place in messages array

### Challenge 3: Database File Sharing
- **Problem**: iOS sandboxing restricts file access
- **Solution**: Use UIActivityViewController (ShareSheet) to share database file

### Challenge 4: HTTP to Localhost
- **Problem**: iOS blocks non-HTTPS connections by default
- **Solution**: Add NSAppTransportSecurity with NSAllowsArbitraryLoads in Info.plist

## Testing Checklist

- [ ] Add server with valid URL
- [ ] Test connection to Ollama server
- [ ] Create new conversation
- [ ] Send message and receive streaming response
- [ ] Select different models
- [ ] Delete conversation
- [ ] Delete server
- [ ] Export database via share sheet
- [ ] Test on simulator
- [ ] Test on physical device

## Files Created

1. **Models.swift** - Data models and API structures
2. **DatabaseManager.swift** - SQLite persistence layer (332 lines)
3. **OllamaService.swift** - API service with streaming support (148 lines)
4. **ViewModels.swift** - State management layer (147 lines)
5. **ServerListView.swift** - Server management UI (119 lines)
6. **ConversationListView.swift** - Conversation list UI (72 lines)
7. **ChatView.swift** - Main chat interface (144 lines)
8. **DatabaseExportView.swift** - Database export UI (57 lines)
9. **OllamaChatApp.swift** - App entry point (13 lines)
10. **Info.plist** - App configuration with security settings
11. **README.md** - Comprehensive documentation with compilation instructions

Total: ~1,100 lines of Swift code

## Future Improvements

If time permits or for future iterations:
- Add markdown rendering for messages
- Implement code syntax highlighting
- Add conversation search
- Support for image inputs (multimodal models)
- Message editing and regeneration
- Export individual conversations as text/JSON
- Support for model parameters (temperature, top_p, etc.)
- Conversation branching
- iPad-optimized layout with split views
