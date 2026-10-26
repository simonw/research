//
//  Models.swift
//  OllamaChat
//
//  Data models for the Ollama chat application
//

import Foundation

// MARK: - Server Model
struct Server: Identifiable, Codable {
    let id: UUID
    var url: String
    var alias: String
    let createdAt: Date

    init(id: UUID = UUID(), url: String, alias: String, createdAt: Date = Date()) {
        self.id = id
        self.url = url
        self.alias = alias
        self.createdAt = createdAt
    }
}

// MARK: - Conversation Model
struct Conversation: Identifiable, Codable {
    let id: UUID
    var serverId: UUID
    var title: String
    let createdAt: Date
    var updatedAt: Date

    init(id: UUID = UUID(), serverId: UUID, title: String, createdAt: Date = Date(), updatedAt: Date = Date()) {
        self.id = id
        self.serverId = serverId
        self.title = title
        self.createdAt = createdAt
        self.updatedAt = updatedAt
    }
}

// MARK: - Message Model
struct Message: Identifiable, Codable {
    let id: UUID
    var conversationId: UUID
    var role: MessageRole
    var content: String
    let timestamp: Date

    init(id: UUID = UUID(), conversationId: UUID, role: MessageRole, content: String, timestamp: Date = Date()) {
        self.id = id
        self.conversationId = conversationId
        self.role = role
        self.content = content
        self.timestamp = timestamp
    }
}

enum MessageRole: String, Codable {
    case user
    case assistant
    case system
}

// MARK: - Ollama API Models
struct OllamaMessage: Codable {
    let role: String
    let content: String
}

struct OllamaChatRequest: Codable {
    let model: String
    let messages: [OllamaMessage]
    let stream: Bool
}

struct OllamaChatResponse: Codable {
    let model: String?
    let message: OllamaMessage?
    let done: Bool
    let createdAt: String?

    enum CodingKeys: String, CodingKey {
        case model
        case message
        case done
        case createdAt = "created_at"
    }
}

struct OllamaModelsResponse: Codable {
    let models: [OllamaModel]
}

struct OllamaModel: Codable {
    let name: String
    let modifiedAt: String?

    enum CodingKeys: String, CodingKey {
        case name
        case modifiedAt = "modified_at"
    }
}
