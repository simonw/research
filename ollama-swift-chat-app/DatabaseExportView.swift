//
//  DatabaseExportView.swift
//  OllamaChat
//
//  View for exporting the SQLite database
//

import SwiftUI

struct DatabaseExportView: View {
    @State private var showingShareSheet = false
    @State private var databaseURL: URL?

    var body: some View {
        Form {
            Section(header: Text("Export")) {
                Button(action: exportDatabase) {
                    HStack {
                        Image(systemName: "square.and.arrow.up")
                        Text("Export Database")
                    }
                }

                Text("Export the entire conversation database as a SQLite file. You can then import it on another device or use it for backup.")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Section(header: Text("Database Info")) {
                HStack {
                    Text("Location")
                    Spacer()
                    Text("Documents")
                        .foregroundColor(.secondary)
                }

                HStack {
                    Text("File Name")
                    Spacer()
                    Text("OllamaChat.sqlite")
                        .foregroundColor(.secondary)
                        .font(.caption)
                }
            }
        }
        .navigationTitle("Export Database")
        .sheet(isPresented: $showingShareSheet) {
            if let url = databaseURL {
                ShareSheet(items: [url])
            }
        }
    }

    private func exportDatabase() {
        databaseURL = DatabaseManager.shared.getDatabaseURL()
        showingShareSheet = true
    }
}

struct ShareSheet: UIViewControllerRepresentable {
    let items: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        let controller = UIActivityViewController(activityItems: items, applicationActivities: nil)
        return controller
    }

    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}
