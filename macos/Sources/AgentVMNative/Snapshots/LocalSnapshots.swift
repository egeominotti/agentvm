import SwiftUI

struct NativeLocalSnapshots: View {
    @ObservedObject var store: NativeStore
    let query: String
    let kind: NativeSnapshotKind
    @Binding var selected: Set<String>
    @State private var bulkOperation: String?
    @State private var confirmDelete = false

    private var shown: [JSON] { NativeSnapshotPolicy.filtered(store.snapshots, query: query, kind: kind) }
    private var picked: [String] { NativeSnapshotPolicy.selected(shown, ids: selected) }
    private var s3Ready: Bool { !store.settings["s3"].isNull }

    var body: some View {
        VStack(spacing: 8) {
            ViewThatFits(in: .horizontal) {
                HStack {
                    selectionControls
                    bulkActions
                }.fixedSize(horizontal: true, vertical: false)
                VStack(alignment: .leading, spacing: 8) {
                    HStack { selectionControls; Spacer() }
                    HStack { bulkActions; Spacer() }
                }
            }.frame(maxWidth: .infinity, alignment: .leading)
            if shown.isEmpty {
                ContentUnavailableView(store.snapshots.isEmpty ? "No snapshots yet" : "No matching snapshots",
                                       systemImage: "camera", description: Text(store.snapshots.isEmpty
                                        ? "Take a snapshot from a running machine or enable automatic snapshots in Settings."
                                        : "Change the search or kind filter to find a snapshot."))
            } else {
                List {
                    ForEach(NativeSnapshotPolicy.groups(shown)) { group in
                        Section {
                            ForEach(group.snapshots, id: \.id) { snapshot in
                                NativeSnapshotRow(store: store, snapshot: snapshot,
                                                  label: NativeSnapshotPolicy.label(snapshot, title: group.title),
                                                  selection: Binding(get: { selected.contains(snapshot.id) },
                                                                     set: { on in
                                    if on { selected.insert(snapshot.id) } else { selected.remove(snapshot.id) }
                                }), s3Ready: s3Ready)
                            }
                        } header: {
                            HStack {
                                Text(group.title).font(.headline)
                                Text(URL(fileURLWithPath: group.repo).lastPathComponent).foregroundStyle(.secondary)
                                Spacer()
                                Text("\(group.snapshots.count) snapshots").font(.caption)
                            }.help("Restores into \(group.repo) on a branch of its own")
                        }
                    }
                }.listStyle(.inset)
            }
        }
        .confirmationDialog("Delete \(picked.count) selected snapshots permanently?", isPresented: $confirmDelete) {
            Button("Delete \(picked.count) snapshots", role: .destructive) { Task { await bulk(backup: false, ids: picked) } }
        }
    }

    @ViewBuilder private var selectionControls: some View {
        Toggle("Select all shown", isOn: Binding(get: { !shown.isEmpty && picked.count == shown.count },
                                                 set: { selected = $0 ? Set(shown.map(\.id)) : [] }))
            .toggleStyle(.checkbox)
        if !picked.isEmpty {
            Text("\(picked.count) selected").font(.callout)
            Button("Clear") { selected.removeAll() }
        }
    }

    @ViewBuilder private var bulkActions: some View {
        if !picked.isEmpty {
            if s3Ready {
                Button("Back up to S3", systemImage: "icloud.and.arrow.up") { Task { await bulk(backup: true, ids: picked) } }
                    .disabled(bulkOperation != nil)
            }
            Button("Delete…", role: .destructive) { confirmDelete = true }.disabled(bulkOperation != nil)
        }
        if let bulkOperation { ProgressView().controlSize(.small); Text(bulkOperation).font(.caption) }
    }

    private func bulk(backup: Bool, ids: [String]) async {
        guard bulkOperation == nil else { return }
        bulkOperation = backup ? "Backing up…" : "Deleting…"
        defer { bulkOperation = nil }
        let completed = await store.performSnapshotBatch(ids: ids, backup: backup)
        selected.subtract(completed)
    }
}
