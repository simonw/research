//
//  ServerListView.swift
//  OllamaChat
//
//  View for managing Ollama servers
//

import SwiftUI

struct ServerListView: View {
    @StateObject private var viewModel = ServersViewModel()
    @State private var showingAddServer = false

    var body: some View {
        NavigationView {
            List {
                ForEach(viewModel.servers) { server in
                    NavigationLink(destination: ConversationListView(server: server)) {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(server.alias)
                                .font(.headline)
                            Text(server.url)
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                    }
                }
                .onDelete { indexSet in
                    indexSet.forEach { index in
                        viewModel.deleteServer(viewModel.servers[index])
                    }
                }
            }
            .navigationTitle("Ollama Servers")
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button(action: { showingAddServer = true }) {
                        Image(systemName: "plus")
                    }
                }
                ToolbarItem(placement: .navigationBarLeading) {
                    NavigationLink(destination: SettingsView()) {
                        Image(systemName: "gear")
                    }
                }
            }
            .sheet(isPresented: $showingAddServer) {
                AddServerView(viewModel: viewModel)
            }

            // Default view when no server is selected
            VStack {
                Image(systemName: "server.rack")
                    .font(.system(size: 60))
                    .foregroundColor(.secondary)
                Text("Select a server to start chatting")
                    .font(.headline)
                    .foregroundColor(.secondary)
            }
        }
    }
}

struct AddServerView: View {
    @ObservedObject var viewModel: ServersViewModel
    @Environment(\.presentationMode) var presentationMode

    @State private var url: String = "http://localhost:11434"
    @State private var alias: String = ""
    @State private var testingConnection = false
    @State private var connectionStatus: String?

    var body: some View {
        NavigationView {
            Form {
                Section(header: Text("Server Details")) {
                    TextField("Server URL", text: $url)
                        .autocapitalization(.none)
                        .disableAutocorrection(true)
                        .keyboardType(.URL)

                    TextField("Alias (optional)", text: $alias)
                }

                Section {
                    Button(action: testConnection) {
                        HStack {
                            Text("Test Connection")
                            if testingConnection {
                                Spacer()
                                ProgressView()
                            }
                        }
                    }
                    .disabled(url.isEmpty || testingConnection)

                    if let status = connectionStatus {
                        Text(status)
                            .font(.caption)
                            .foregroundColor(status.contains("Success") ? .green : .red)
                    }
                }
            }
            .navigationTitle("Add Server")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        presentationMode.wrappedValue.dismiss()
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        saveServer()
                    }
                    .disabled(url.isEmpty)
                }
            }
        }
    }

    private func testConnection() {
        testingConnection = true
        connectionStatus = nil

        // Create a temporary server for testing
        let tempServer = Server(url: url, alias: alias.isEmpty ? "Test" : alias)

        viewModel.testConnection(for: tempServer) { success in
            testingConnection = false
            connectionStatus = success ? "✓ Connection successful" : "✗ Connection failed"
        }
    }

    private func saveServer() {
        let finalAlias = alias.isEmpty ? url : alias
        viewModel.addServer(url: url, alias: finalAlias)
        presentationMode.wrappedValue.dismiss()
    }
}

struct SettingsView: View {
    var body: some View {
        Form {
            Section(header: Text("About")) {
                HStack {
                    Text("Version")
                    Spacer()
                    Text("1.0.0")
                        .foregroundColor(.secondary)
                }
            }

            Section(header: Text("Database")) {
                NavigationLink(destination: DatabaseExportView()) {
                    Text("Export Database")
                }
            }

            Section(header: Text("Info")) {
                Text("OllamaChat allows you to chat with your local Ollama instances.")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
        }
        .navigationTitle("Settings")
    }
}
