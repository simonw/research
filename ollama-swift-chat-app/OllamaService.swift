//
//  OllamaService.swift
//  OllamaChat
//
//  Service for communicating with Ollama API (streaming chat support)
//

import Foundation
import Combine

class OllamaService {
    static let shared = OllamaService()

    private init() {}

    // MARK: - Streaming Chat
    func streamChat(
        serverUrl: String,
        model: String,
        messages: [Message],
        onChunk: @escaping (String) -> Void,
        onComplete: @escaping () -> Void,
        onError: @escaping (Error) -> Void
    ) {
        guard let url = URL(string: "\(serverUrl)/api/chat") else {
            onError(OllamaError.invalidURL)
            return
        }

        // Convert our messages to Ollama format
        let ollamaMessages = messages.map { message in
            OllamaMessage(role: message.role.rawValue, content: message.content)
        }

        let requestBody = OllamaChatRequest(
            model: model,
            messages: ollamaMessages,
            stream: true
        )

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        do {
            request.httpBody = try JSONEncoder().encode(requestBody)
        } catch {
            onError(error)
            return
        }

        let task = URLSession.shared.dataTask(with: request) { data, response, error in
            if let error = error {
                DispatchQueue.main.async {
                    onError(error)
                }
                return
            }

            guard let data = data else {
                DispatchQueue.main.async {
                    onError(OllamaError.noData)
                }
                return
            }

            // Parse the streaming response (newline-delimited JSON)
            let responseString = String(data: data, encoding: .utf8) ?? ""
            let lines = responseString.components(separatedBy: "\n")

            var fullContent = ""

            for line in lines {
                guard !line.isEmpty else { continue }

                if let lineData = line.data(using: .utf8),
                   let response = try? JSONDecoder().decode(OllamaChatResponse.self, from: lineData) {

                    if let message = response.message {
                        fullContent += message.content
                        DispatchQueue.main.async {
                            onChunk(message.content)
                        }
                    }

                    if response.done {
                        DispatchQueue.main.async {
                            onComplete()
                        }
                    }
                }
            }
        }

        task.resume()
    }

    // MARK: - Get Available Models
    func getModels(
        serverUrl: String,
        completion: @escaping (Result<[String], Error>) -> Void
    ) {
        guard let url = URL(string: "\(serverUrl)/api/tags") else {
            completion(.failure(OllamaError.invalidURL))
            return
        }

        var request = URLRequest(url: url)
        request.httpMethod = "GET"

        let task = URLSession.shared.dataTask(with: request) { data, response, error in
            if let error = error {
                DispatchQueue.main.async {
                    completion(.failure(error))
                }
                return
            }

            guard let data = data else {
                DispatchQueue.main.async {
                    completion(.failure(OllamaError.noData))
                }
                return
            }

            do {
                let response = try JSONDecoder().decode(OllamaModelsResponse.self, from: data)
                let modelNames = response.models.map { $0.name }
                DispatchQueue.main.async {
                    completion(.success(modelNames))
                }
            } catch {
                DispatchQueue.main.async {
                    completion(.failure(error))
                }
            }
        }

        task.resume()
    }

    // MARK: - Test Server Connection
    func testConnection(
        serverUrl: String,
        completion: @escaping (Bool) -> Void
    ) {
        getModels(serverUrl: serverUrl) { result in
            switch result {
            case .success:
                completion(true)
            case .failure:
                completion(false)
            }
        }
    }
}

// MARK: - Errors
enum OllamaError: Error, LocalizedError {
    case invalidURL
    case noData
    case decodingError

    var errorDescription: String? {
        switch self {
        case .invalidURL:
            return "Invalid server URL"
        case .noData:
            return "No data received from server"
        case .decodingError:
            return "Failed to decode response"
        }
    }
}
