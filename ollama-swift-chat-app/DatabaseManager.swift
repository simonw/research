//
//  DatabaseManager.swift
//  OllamaChat
//
//  SQLite database manager for persisting servers, conversations, and messages
//

import Foundation
import SQLite3

class DatabaseManager {
    static let shared = DatabaseManager()
    private var db: OpaquePointer?

    private init() {
        openDatabase()
        createTables()
    }

    // MARK: - Database Setup
    private func openDatabase() {
        let fileURL = try! FileManager.default
            .url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
            .appendingPathComponent("OllamaChat.sqlite")

        if sqlite3_open(fileURL.path, &db) != SQLITE_OK {
            print("Error opening database")
        }
    }

    private func createTables() {
        let createServersTable = """
        CREATE TABLE IF NOT EXISTS servers (
            id TEXT PRIMARY KEY,
            url TEXT NOT NULL,
            alias TEXT NOT NULL,
            created_at REAL NOT NULL
        );
        """

        let createConversationsTable = """
        CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            server_id TEXT NOT NULL,
            title TEXT NOT NULL,
            created_at REAL NOT NULL,
            updated_at REAL NOT NULL,
            FOREIGN KEY(server_id) REFERENCES servers(id) ON DELETE CASCADE
        );
        """

        let createMessagesTable = """
        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            timestamp REAL NOT NULL,
            FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );
        """

        executeSQL(createServersTable)
        executeSQL(createConversationsTable)
        executeSQL(createMessagesTable)
    }

    private func executeSQL(_ sql: String) {
        var statement: OpaquePointer?
        if sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK {
            if sqlite3_step(statement) != SQLITE_DONE {
                print("Error executing SQL: \(sql)")
            }
        }
        sqlite3_finalize(statement)
    }

    // MARK: - Server Operations
    func saveServer(_ server: Server) {
        let sql = "INSERT OR REPLACE INTO servers (id, url, alias, created_at) VALUES (?, ?, ?, ?);"
        var statement: OpaquePointer?

        if sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK {
            sqlite3_bind_text(statement, 1, server.id.uuidString, -1, nil)
            sqlite3_bind_text(statement, 2, server.url, -1, nil)
            sqlite3_bind_text(statement, 3, server.alias, -1, nil)
            sqlite3_bind_double(statement, 4, server.createdAt.timeIntervalSince1970)

            if sqlite3_step(statement) != SQLITE_DONE {
                print("Error saving server")
            }
        }
        sqlite3_finalize(statement)
    }

    func getAllServers() -> [Server] {
        let sql = "SELECT id, url, alias, created_at FROM servers ORDER BY created_at DESC;"
        var statement: OpaquePointer?
        var servers: [Server] = []

        if sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK {
            while sqlite3_step(statement) == SQLITE_ROW {
                let idString = String(cString: sqlite3_column_text(statement, 0))
                let url = String(cString: sqlite3_column_text(statement, 1))
                let alias = String(cString: sqlite3_column_text(statement, 2))
                let createdAt = sqlite3_column_double(statement, 3)

                if let id = UUID(uuidString: idString) {
                    servers.append(Server(id: id, url: url, alias: alias, createdAt: Date(timeIntervalSince1970: createdAt)))
                }
            }
        }
        sqlite3_finalize(statement)
        return servers
    }

    func deleteServer(_ id: UUID) {
        let sql = "DELETE FROM servers WHERE id = ?;"
        var statement: OpaquePointer?

        if sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK {
            sqlite3_bind_text(statement, 1, id.uuidString, -1, nil)
            sqlite3_step(statement)
        }
        sqlite3_finalize(statement)
    }

    // MARK: - Conversation Operations
    func saveConversation(_ conversation: Conversation) {
        let sql = "INSERT OR REPLACE INTO conversations (id, server_id, title, created_at, updated_at) VALUES (?, ?, ?, ?, ?);"
        var statement: OpaquePointer?

        if sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK {
            sqlite3_bind_text(statement, 1, conversation.id.uuidString, -1, nil)
            sqlite3_bind_text(statement, 2, conversation.serverId.uuidString, -1, nil)
            sqlite3_bind_text(statement, 3, conversation.title, -1, nil)
            sqlite3_bind_double(statement, 4, conversation.createdAt.timeIntervalSince1970)
            sqlite3_bind_double(statement, 5, conversation.updatedAt.timeIntervalSince1970)

            if sqlite3_step(statement) != SQLITE_DONE {
                print("Error saving conversation")
            }
        }
        sqlite3_finalize(statement)
    }

    func getConversations(forServer serverId: UUID) -> [Conversation] {
        let sql = "SELECT id, server_id, title, created_at, updated_at FROM conversations WHERE server_id = ? ORDER BY updated_at DESC;"
        var statement: OpaquePointer?
        var conversations: [Conversation] = []

        if sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK {
            sqlite3_bind_text(statement, 1, serverId.uuidString, -1, nil)

            while sqlite3_step(statement) == SQLITE_ROW {
                let idString = String(cString: sqlite3_column_text(statement, 0))
                let serverIdString = String(cString: sqlite3_column_text(statement, 1))
                let title = String(cString: sqlite3_column_text(statement, 2))
                let createdAt = sqlite3_column_double(statement, 3)
                let updatedAt = sqlite3_column_double(statement, 4)

                if let id = UUID(uuidString: idString), let serverId = UUID(uuidString: serverIdString) {
                    conversations.append(Conversation(
                        id: id,
                        serverId: serverId,
                        title: title,
                        createdAt: Date(timeIntervalSince1970: createdAt),
                        updatedAt: Date(timeIntervalSince1970: updatedAt)
                    ))
                }
            }
        }
        sqlite3_finalize(statement)
        return conversations
    }

    func deleteConversation(_ id: UUID) {
        let sql = "DELETE FROM conversations WHERE id = ?;"
        var statement: OpaquePointer?

        if sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK {
            sqlite3_bind_text(statement, 1, id.uuidString, -1, nil)
            sqlite3_step(statement)
        }
        sqlite3_finalize(statement)
    }

    // MARK: - Message Operations
    func saveMessage(_ message: Message) {
        let sql = "INSERT INTO messages (id, conversation_id, role, content, timestamp) VALUES (?, ?, ?, ?, ?);"
        var statement: OpaquePointer?

        if sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK {
            sqlite3_bind_text(statement, 1, message.id.uuidString, -1, nil)
            sqlite3_bind_text(statement, 2, message.conversationId.uuidString, -1, nil)
            sqlite3_bind_text(statement, 3, message.role.rawValue, -1, nil)
            sqlite3_bind_text(statement, 4, message.content, -1, nil)
            sqlite3_bind_double(statement, 5, message.timestamp.timeIntervalSince1970)

            if sqlite3_step(statement) != SQLITE_DONE {
                print("Error saving message")
            }
        }
        sqlite3_finalize(statement)
    }

    func getMessages(forConversation conversationId: UUID) -> [Message] {
        let sql = "SELECT id, conversation_id, role, content, timestamp FROM messages WHERE conversation_id = ? ORDER BY timestamp ASC;"
        var statement: OpaquePointer?
        var messages: [Message] = []

        if sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK {
            sqlite3_bind_text(statement, 1, conversationId.uuidString, -1, nil)

            while sqlite3_step(statement) == SQLITE_ROW {
                let idString = String(cString: sqlite3_column_text(statement, 0))
                let conversationIdString = String(cString: sqlite3_column_text(statement, 1))
                let roleString = String(cString: sqlite3_column_text(statement, 2))
                let content = String(cString: sqlite3_column_text(statement, 3))
                let timestamp = sqlite3_column_double(statement, 4)

                if let id = UUID(uuidString: idString),
                   let conversationId = UUID(uuidString: conversationIdString),
                   let role = MessageRole(rawValue: roleString) {
                    messages.append(Message(
                        id: id,
                        conversationId: conversationId,
                        role: role,
                        content: content,
                        timestamp: Date(timeIntervalSince1970: timestamp)
                    ))
                }
            }
        }
        sqlite3_finalize(statement)
        return messages
    }

    // MARK: - Database Export
    func getDatabaseURL() -> URL {
        return try! FileManager.default
            .url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
            .appendingPathComponent("OllamaChat.sqlite")
    }
}
