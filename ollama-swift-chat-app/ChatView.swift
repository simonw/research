//
//  ChatView.swift
//  OllamaChat
//
//  Main chat interface with streaming support
//

import SwiftUI

struct ChatView: View {
    let conversation: Conversation
    let server: Server

    @StateObject private var viewModel = ChatViewModel()
    @State private var showingModelPicker = false

    var body: some View {
        VStack(spacing: 0) {
            // Model selector
            HStack {
                Text("Model:")
                    .font(.caption)
                    .foregroundColor(.secondary)

                Button(action: { showingModelPicker = true }) {
                    HStack {
                        Text(viewModel.selectedModel)
                            .font(.caption)
                        Image(systemName: "chevron.down")
                            .font(.caption2)
                    }
                }
                .disabled(viewModel.availableModels.isEmpty)

                Spacer()

                if viewModel.isLoading {
                    ProgressView()
                        .scaleEffect(0.8)
                }
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
            .background(Color(.systemGray6))

            Divider()

            // Messages list
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(spacing: 12) {
                        ForEach(viewModel.messages) { message in
                            MessageBubble(message: message)
                                .id(message.id)
                        }
                    }
                    .padding()
                }
                .onChange(of: viewModel.messages.count) { _ in
                    if let lastMessage = viewModel.messages.last {
                        withAnimation {
                            proxy.scrollTo(lastMessage.id, anchor: .bottom)
                        }
                    }
                }
            }

            Divider()

            // Input area
            HStack(alignment: .bottom, spacing: 12) {
                TextField("Message", text: $viewModel.currentInput, axis: .vertical)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                    .lineLimit(1...6)
                    .disabled(viewModel.isLoading)

                Button(action: { viewModel.sendMessage() }) {
                    Image(systemName: "arrow.up.circle.fill")
                        .font(.system(size: 32))
                        .foregroundColor(viewModel.currentInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || viewModel.isLoading ? .gray : .blue)
                }
                .disabled(viewModel.currentInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || viewModel.isLoading)
            }
            .padding()
            .background(Color(.systemBackground))
        }
        .navigationTitle(conversation.title)
        .navigationBarTitleDisplayMode(.inline)
        .onAppear {
            viewModel.setup(conversation: conversation, server: server)
        }
        .sheet(isPresented: $showingModelPicker) {
            ModelPickerView(
                models: viewModel.availableModels,
                selectedModel: $viewModel.selectedModel,
                isPresented: $showingModelPicker
            )
        }
    }
}

struct MessageBubble: View {
    let message: Message

    var body: some View {
        HStack {
            if message.role == .user {
                Spacer(minLength: 60)
            }

            VStack(alignment: message.role == .user ? .trailing : .leading, spacing: 4) {
                Text(message.content)
                    .padding(12)
                    .background(message.role == .user ? Color.blue : Color(.systemGray5))
                    .foregroundColor(message.role == .user ? .white : .primary)
                    .cornerRadius(16)

                Text(formatTime(message.timestamp))
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }

            if message.role == .assistant {
                Spacer(minLength: 60)
            }
        }
    }

    private func formatTime(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.timeStyle = .short
        return formatter.string(from: date)
    }
}

struct ModelPickerView: View {
    let models: [String]
    @Binding var selectedModel: String
    @Binding var isPresented: Bool

    var body: some View {
        NavigationView {
            List(models, id: \.self) { model in
                Button(action: {
                    selectedModel = model
                    isPresented = false
                }) {
                    HStack {
                        Text(model)
                            .foregroundColor(.primary)
                        Spacer()
                        if model == selectedModel {
                            Image(systemName: "checkmark")
                                .foregroundColor(.blue)
                        }
                    }
                }
            }
            .navigationTitle("Select Model")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        isPresented = false
                    }
                }
            }
        }
    }
}
