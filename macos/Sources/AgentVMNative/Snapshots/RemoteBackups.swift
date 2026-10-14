import SwiftUI

private extension JSON {
    var backupID: String { self["snapshot"]["id"].text }
}

struct NativeRemoteBackups: View {
    @ObservedObject var store: NativeStore
    let query: String
    @State private var removing: JSON = .null
    @State private var confirmDelete = false

    private var shown: [JSON] {
        let query = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return store.backups.filter {
            query.isEmpty || ($0["snapshot"]["name"].text + " " + $0["snapshot"]["repo"].text).lowercased().contains(query)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if let error = store.backupsError {
                VStack(alignment: .leading, spacing: 10) {
                    Label(error, systemImage: "exclamationmark.icloud").foregroundStyle(.orange)
                        .fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
                    HStack {
                        Button("Retry connection", systemImage: "arrow.clockwise") { Task { await store.refreshSnapshots() } }
                            .disabled(store.backupsLoading)
                        Button("Check S3 settings", action: showSettings)
                    }
                }.padding().frame(maxWidth: .infinity, alignment: .leading)
                    .background(.quaternary, in: RoundedRectangle(cornerRadius: 10))
            }
            if store.backupsLoading { ProgressView("Reading the S3 bucket…").controlSize(.small) }
            if store.settings["s3"].isNull {
                VStack(spacing: 16) {
                    ContentUnavailableView("Keep copies off this Mac", systemImage: "externaldrive.badge.icloud",
                                           description: Text("Connect an S3-compatible bucket to back up and restore snapshots."))
                    Button("Set up S3 backups", action: showSettings)
                }
            } else if store.backups.isEmpty && (store.backupsError != nil || store.backupsLoading) {
                Spacer()
            } else if shown.isEmpty {
                ContentUnavailableView(store.backups.isEmpty ? "No backups yet" : "No matching backups",
                                       systemImage: "icloud", description: Text("Back up a local snapshot to S3, or change the search."))
            } else {
                List(shown, id: \.backupID) { backup in
                    NativeRemoteBackupRow(store: store, backup: backup) {
                        removing = backup["snapshot"]
                        confirmDelete = true
                    }
                }.listStyle(.inset)
            }
        }
        .confirmationDialog("Delete \(removing["name"].text) from S3?", isPresented: $confirmDelete) {
            Button("Delete from S3", role: .destructive) {
                Task { _ = await store.perform("/api/backups/\(removing.id)", method: "DELETE", success: "Backup deleted from S3") }
            }
        }
    }

    private func showSettings() {
        UserDefaults.standard.set("backups", forKey: "native.settingsSection")
        store.route = .settings
    }
}

private struct NativeRemoteBackupRow: View {
    @ObservedObject var store: NativeStore
    let backup: JSON
    let remove: () -> Void
    private var snapshot: JSON { backup["snapshot"] }

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 14) {
                title.frame(maxWidth: .infinity, alignment: .leading)
                metadata
                actions
            }.frame(minWidth: 660)
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    title
                    Spacer(minLength: 8)
                    actions
                }
                HStack(spacing: 16) { metadata }
            }
        }.padding(.vertical, 6)
    }

    private var title: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(snapshot["name"].text).font(.headline).lineLimit(2)
            Text("Uploaded \(backup["uploaded_at"].text)").font(.caption).foregroundStyle(.secondary).lineLimit(1)
        }
    }

    @ViewBuilder private var metadata: some View {
        Text(URL(fileURLWithPath: snapshot["repo"].text).lastPathComponent)
            .foregroundStyle(.secondary).lineLimit(1).help(snapshot["repo"].text)
        Text(Date(timeIntervalSince1970: snapshot["created_at"].number), style: .relative)
            .font(.caption).foregroundStyle(.secondary)
        Text(NativeSettingsPolicy.size(backup["archive_mb"].integer)).font(.callout.monospacedDigit())
    }

    @ViewBuilder private var actions: some View {
        Button(backup["local"].flag ? "On this Mac" : "Bring here") {
            Task { _ = await store.perform("/api/backups/\(snapshot.id)/restore", success: "Backup is now on this Mac") }
        }.disabled(backup["local"].flag || store.busy.contains("/api/backups/\(snapshot.id)/restore"))
        Button(role: .destructive, action: remove) { Image(systemName: "trash") }
            .buttonStyle(.borderless).help("Delete from S3")
    }
}
