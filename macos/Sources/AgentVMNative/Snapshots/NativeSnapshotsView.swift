import SwiftUI

struct NativeSnapshotsView: View {
    @ObservedObject var store: NativeStore
    @Environment(\.scenePhase) private var scenePhase
    @State private var tab = "local"
    @State private var query = ""
    @State private var kind: NativeSnapshotKind = .all
    @State private var selected: Set<String> = []
    @State private var importing = false
    @State private var cloudListed = false

    var body: some View {
        GeometryReader { viewport in
            content.frame(width: viewport.size.width, height: viewport.size.height, alignment: .topLeading)
        }
            .task(id: "\(tab)-\(scenePhase == .active)") {
                guard scenePhase == .active else { return }
                while !Task.isCancelled {
                    let cloud = !cloudListed || tab == "s3"
                    await store.refreshSnapshots(includeBackups: cloud)
                    if cloud { cloudListed = true }
                    let compacting = store.snapshots.contains { $0["compacting"].flag }
                    let interval: Double = tab == "s3" ? 30 : compacting ? 2 : 15
                    do { try await Task.sleep(for: .seconds(interval)) }
                    catch { break }
                }
            }
            .onChange(of: store.snapshots.map(\.id)) { _, ids in selected.formIntersection(ids) }
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Snapshots").font(.largeTitle.bold())
                    Text("Whole VMs saved with files, packages and Claude's conversation.").foregroundStyle(.secondary)
                }
                Spacer()
                Button(importing ? "Importing…" : "Import…", systemImage: "square.and.arrow.down") {
                    if let file = NativeFilePanels.archive() { Task { await importArchive(file) } }
                }.disabled(importing)
                Button("Refresh", systemImage: "arrow.clockwise") { Task { await store.refreshSnapshots() } }
            }
            HStack(spacing: 30) {
                statistic("Snapshots", value: "\(store.snapshots.count)")
                statistic("On disk", value: NativeSettingsPolicy.size(store.storage["snapshots_mb"].integer))
                statistic("Machines", value: "\(Set(store.snapshots.map { $0["source_task"].text }).count)")
                statistic("S3 backups", value: store.settings["s3"].isNull ? "Not set up"
                          : store.backupsError != nil ? "Unavailable"
                          : store.backupsLoading && store.backups.isEmpty ? "Reading…" : "\(store.backups.count)")
                Spacer()
            }.padding().background(.quaternary, in: RoundedRectangle(cornerRadius: 12))
            Text("Local size estimates the space freed on deletion. Portable archives include shared disk data and can be larger.")
                .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            HStack {
                Picker("Location", selection: $tab) {
                    Text("On this Mac").tag("local")
                    Text("In S3").tag("s3")
                }.labelsHidden().pickerStyle(.segmented).frame(width: 240)
                Spacer()
            }
            HStack {
                TextField("Search by name or repository", text: $query).textFieldStyle(.roundedBorder)
                if tab == "local" {
                    Picker("Kind", selection: $kind) {
                        ForEach(NativeSnapshotKind.allCases) { Text($0.title).tag($0) }
                    }.labelsHidden().frame(width: 150)
                }
            }
            GeometryReader { area in
                Group {
                    if tab == "local" {
                        NativeLocalSnapshots(store: store, query: query, kind: kind, selected: $selected)
                    } else {
                        NativeRemoteBackups(store: store, query: query)
                    }
                }.frame(width: area.size.width, height: area.size.height, alignment: .topLeading)
                    .clipped()
            }
        }.padding()
    }

    private func statistic(_ title: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title).font(.caption).foregroundStyle(.secondary)
            Text(value).font(.title2.monospacedDigit())
        }
    }

    private func importArchive(_ file: URL) async {
        importing = true
        store.error = nil
        store.notice = "Importing \(file.lastPathComponent)…"
        let scoped = file.startAccessingSecurityScopedResource()
        defer {
            importing = false
            if scoped { file.stopAccessingSecurityScopedResource() }
        }
        do {
            let imported = try await store.api.upload("/api/snapshots/import", file: file)
            store.notice = "Imported \(imported["name"].text.isEmpty ? file.lastPathComponent : imported["name"].text)"
            await store.refreshSnapshots()
        } catch { store.error = error.localizedDescription; store.notice = nil }
    }
}
