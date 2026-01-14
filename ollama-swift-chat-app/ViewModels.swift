//
//  ViewModels.swift
//  OllamaChat
//
//  View models for managing app state
//

import Foundation
import Combine

// MARK: - Servers ViewModel
class ServersViewModel: ObservableObject {
    @Published var servers: [Server] = []

    init() {
        loadServers()
    }

    func loadServers() {
        servers = DatabaseManager.shared.getAllServers()
    }

    func addServer(url: String, alias: String) {
        let server = Server(url: url, alias: alias)
        DatabaseManager.shared.saveServer(server)
        loadServers()
    }

    func deleteServer(_ server: Server) {
        DatabaseManager.shared.deleteServer(server.id)
        loadServers()
    }

    func testConnection(for server: Server, completion: @escaping (Bool) -> Void) {
        OllamaService.shared.testConnection(serverUrl: server.url, completion: completion)
    }
}

// MARK: - Conversations ViewModel
class ConversationsViewModel: ObservableObject {
    @Published var conversations: [Conversation] = []
    var currentServer: Server?

    func loadConversations(for server: Server) {
        currentServer = server
        conversations = DatabaseManager.shared.getConversations(forServer: server.id)
    }

    func createConversation(title: String) {
        guard let server = currentServer else { return }

        let conversation = Conversation(serverId: server.id, title: title)
        DatabaseManager.shared.saveConversation(conversation)
        loadConversations(for: server)
    }

    func deleteConversation(_ conversation: Conversation) {
        DatabaseManager.shared.deleteConversation(conversation.id)
        if let server = currentServer {
            loadConversations(for: server)
        }
    }

    func updateConversationTimestamp(_ conversation: Conversation) {
        var updated = conversation
        updated.updatedAt = Date()
        DatabaseManager.shared.saveConversation(updated)
        if let server = currentServer {
            loadConversations(for: server)
        }
    }
}

// MARK: - Chat ViewModel
class ChatViewModel: ObservableObject {
    @Published var messages: [Message] = []
    @Published var currentInput: String = ""
    @Published var isLoading: Bool = false
    @Published var selectedModel: String = "llama2"
    @Published var availableModels: [String] = []
    @Published var streamingContent: String = ""

    var currentConversation: Conversation?
    var currentServer: Server?

    func setup(conversation: Conversation, server: Server) {
        currentConversation = conversation
        currentServer = server
        loadMessages()
        loadAvailableModels()
    }

    func loadMessages() {
        guard let conversation = currentConversation else { return }
        messages = DatabaseManager.shared.getMessages(forConversation: conversation.id)
    }

    func loadAvailableModels() {
        guard let server = currentServer else { return }

        OllamaService.shared.getModels(serverUrl: server.url) { [weak self] result in
            switch result {
            case .success(let models):
                self?.availableModels = models
                if let firstModel = models.first, self?.selectedModel == "llama2" && !models.contains("llama2") {
                    self?.selectedModel = firstModel
                }
            case .failure(let error):
                print("Failed to load models: \(error)")
            }
        }
    }

    func sendMessage() {
        guard !currentInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              let conversation = currentConversation,
              let server = currentServer else { return }

        let userMessage = Message(
            conversationId: conversation.id,
            role: .user,
            content: currentInput
        )

        // Save user message
        DatabaseManager.shared.saveMessage(userMessage)
        messages.append(userMessage)

        let inputCopy = currentInput
        currentInput = ""
        isLoading = true
        streamingContent = ""

        // Create a temporary assistant message
        let assistantMessageId = UUID()
        let assistantMessage = Message(
            id: assistantMessageId,
            conversationId: conversation.id,
            role: .assistant,
            content: ""
        )
        messages.append(assistantMessage)

        // Stream the response
        OllamaService.shared.streamChat(
            serverUrl: server.url,
            model: selectedModel,
            messages: messages.filter { $0.id != assistantMessageId },
            onChunk: { [weak self] chunk in
                self?.streamingContent += chunk
                if let index = self?.messages.firstIndex(where: { $0.id == assistantMessageId }) {
                    self?.messages[index].content = self?.streamingContent ?? ""
                }
            },
            onComplete: { [weak self] in
                guard let self = self else { return }
                self.isLoading = false

                // Save the complete assistant message
                if let index = self.messages.firstIndex(where: { $0.id == assistantMessageId }) {
                    let finalMessage = self.messages[index]
                    DatabaseManager.shared.saveMessage(finalMessage)
                }

                // Update conversation timestamp
                var updatedConversation = conversation
                updatedConversation.updatedAt = Date()
                DatabaseManager.shared.saveConversation(updatedConversation)

                self.streamingContent = ""
            },
            onError: { [weak self] error in
                self?.isLoading = false
                print("Chat error: \(error)")

                // Remove the temporary message on error
                self?.messages.removeAll { $0.id == assistantMessageId }
                self?.streamingContent = ""
            }
        )
    }
}
