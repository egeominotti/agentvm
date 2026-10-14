import SwiftUI

struct NativeSnapshotRow: View {
    @ObservedObject var store: NativeStore
    let snapshot: JSON
    let label: String
    @Binding var selection: Bool
    let s3Ready: Bool
    @State private var confirmDelete = false
    @State private var exporting = false

    private var base: String { "/api/snapshots/\(snapshot.id)" }
    private var restoring: Bool { store.busy.contains(base + "/restore") }
    private var details: String {
        let model = snapshot["model"].text
        let label = NativeSettingsPolicy.models.first { $0.0 == model }?.1 ?? model
        guard snapshot["cpus"].integer > 0 else { return label }
        return "\(label) · \(snapshot["cpus"].integer) vCPUs · \(NativeSettingsPolicy.size(snapshot["memory_mb"].integer))"
    }

    var body: some View {
        ViewThatFits(in: .horizontal) {
            wideRow.frame(minWidth: 680)
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 12) {
                    selectionToggle
                    rowTitle
                    Spacer(minLength: 8)
                    restoreButton
                    actions
                }
                HStack(spacing: 16) {
                    kindLabel
                    taken
                    Spacer()
                    sizeLabel
                }.padding(.leading, 28)
            }
        }.padding(.vertical, 5)
            .confirmationDialog("Delete \(snapshot["name"].text) permanently?", isPresented: $confirmDelete) {
                Button("Delete snapshot", role: .destructive) { Task { _ = await store.perform(base, method: "DELETE") } }
            }
    }

    private var wideRow: some View {
        HStack(spacing: 14) {
            selectionToggle
            rowTitle.frame(maxWidth: .infinity, alignment: .leading)
            kindLabel.frame(width: 80)
            taken.frame(width: 110)
            sizeLabel.frame(width: 100, alignment: .trailing)
            restoreButton
            actions
        }
    }

    private var selectionToggle: some View {
        Toggle("Select \(label)", isOn: $selection).labelsHidden().toggleStyle(.checkbox)
    }

    private var rowTitle: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label).font(.headline).lineLimit(2).help(snapshot["name"].text)
            Text(details).font(.caption).foregroundStyle(.secondary).lineLimit(2)
        }
    }

    private var kindLabel: some View {
        Text(NativeSnapshotPolicy.kind(snapshot).title).font(.caption).foregroundStyle(.secondary)
    }

    private var taken: some View {
        Text(Date(timeIntervalSince1970: snapshot["created_at"].number), style: .relative)
            .font(.caption).foregroundStyle(.secondary)
            .help(Date(timeIntervalSince1970: snapshot["created_at"].number).formatted())
    }

    private var sizeLabel: some View {
        Text(snapshot["compacting"].flag ? "Compressing…" : snapshot["size_mb"].integer < 1
             ? "< 1 MB" : NativeSettingsPolicy.size(snapshot["size_mb"].integer))
            .font(.caption.monospacedDigit()).help("Estimated space held only by this snapshot's compressed chunks")
    }

    private var restoreButton: some View {
        Button(restoring ? "Starting…" : NativeSnapshotPolicy.kind(snapshot) == .interrupted ? "Resume" : "Restore") {
            Task {
                if let result = await store.restoreSnapshot(snapshot.id), !result["id"].text.isEmpty {
                    store.route = .machine(result["id"].text)
                }
            }
        }.disabled(restoring).help("Starts a new VM from this point; Claude continues its conversation")
    }

    private var actions: some View {
        Menu {
            Button(exporting ? "Exporting…" : "Export archive…") {
                guard let destination = NativeFilePanels.export(named: snapshot.id) else { return }
                Task { await export(to: destination) }
            }.disabled(exporting)
            Button("Back up to S3") { Task { _ = await store.perform(base + "/backup", success: "Snapshot backed up to S3") } }
                .disabled(!s3Ready || store.busy.contains(base + "/backup"))
            Divider()
            Button("Delete…", role: .destructive) { confirmDelete = true }
        } label: { Image(systemName: "ellipsis.circle") }.menuStyle(.borderlessButton).frame(width: 24)
    }

    private func export(to file: URL) async {
        exporting = true
        store.error = nil
        store.notice = "Exporting snapshot…"
        let scoped = file.startAccessingSecurityScopedResource()
        defer { exporting = false; if scoped { file.stopAccessingSecurityScopedResource() } }
        do {
            try await store.api.download(base + "/export", to: file)
            store.notice = "Exported \(file.lastPathComponent)"
        } catch { store.error = error.localizedDescription; store.notice = nil }
    }
}
