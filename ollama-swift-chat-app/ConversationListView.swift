//
//  ConversationListView.swift
//  OllamaChat
//
//  View for managing conversations for a specific server
//

import SwiftUI

struct ConversationListView: View {
    let server: Server

    @StateObject private var viewModel = ConversationsViewModel()
    @State private var showingNewConversation = false

    var body: some View {
        List {
            ForEach(viewModel.conversations) { conversation in
                NavigationLink(destination: ChatView(conversation: conversation, server: server)) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(conversation.title)
                            .font(.headline)
                        Text(formatDate(conversation.updatedAt))
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                }
            }
            .onDelete { indexSet in
                indexSet.forEach { index in
                    viewModel.deleteConversation(viewModel.conversations[index])
                }
            }
        }
        .navigationTitle(server.alias)
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                Button(action: { showingNewConversation = true }) {
                    Image(systemName: "plus")
                }
            }
        }
        .sheet(isPresented: $showingNewConversation) {
            NewConversationView(viewModel: viewModel)
        }
        .onAppear {
            viewModel.loadConversations(for: server)
        }
    }

    private func formatDate(_ date: Date) -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}

struct NewConversationView: View {
    @ObservedObject var viewModel: ConversationsViewModel
    @Environment(\.presentationMode) var presentationMode

    @State private var title: String = ""

    var body: some View {
        NavigationView {
            Form {
                Section(header: Text("Conversation Title")) {
                    TextField("e.g., Code Help", text: $title)
                }
            }
            .navigationTitle("New Conversation")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        presentationMode.wrappedValue.dismiss()
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") {
                        let finalTitle = title.isEmpty ? "New Chat" : title
                        viewModel.createConversation(title: finalTitle)
                        presentationMode.wrappedValue.dismiss()
                    }
                }
            }
        }
    }
}
