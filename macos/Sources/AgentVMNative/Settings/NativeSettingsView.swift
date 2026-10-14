import SwiftUI

struct NativeSettingsView: View {
    @ObservedObject var store: NativeStore
    @AppStorage("native.settingsSection") private var section = "general"
    @State private var query = ""

    var body: some View {
        GeometryReader { geometry in
            let compact = geometry.size.width < 680
            HStack(spacing: 0) {
                if !compact {
                    sidebar
                    Divider()
                }
                VStack(spacing: 0) {
                    if compact {
                        compactNavigation
                        Divider()
                    }
                    // Keep the pane in one structural position so resizing preserves drafts.
                    content
                }
            }
        }
        .task { await store.refresh() }
    }

    private var compactNavigation: some View {
        HStack {
            Picker("Settings page", selection: $section) {
                ForEach(NativeSettingsPolicy.catalog) { Text($0.title).tag($0.id) }
            }.labelsHidden().frame(width: 185)
            TextField("Search settings", text: $query).textFieldStyle(.roundedBorder)
                .onSubmit {
                    if let first = NativeSettingsPolicy.sections(matching: query).first { section = first.id }
                }
        }.padding()
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Settings").font(.title2.bold()).padding(.horizontal)
            TextField("Search settings", text: $query).textFieldStyle(.roundedBorder).padding(.horizontal)
            List(selection: Binding(get: { Optional(section) }, set: { section = $0 ?? "general" })) {
                ForEach(["Workspace", "Accounts & access", "Data", "System"], id: \.self) { group in
                    let sections = NativeSettingsPolicy.sections(matching: query).filter { $0.group == group }
                    if !sections.isEmpty {
                        Section(group) {
                            ForEach(sections) { section in
                                Label(section.title, systemImage: section.symbol).tag(section.id)
                            }
                        }
                    }
                }
            }.listStyle(.sidebar)
        }.frame(width: 220).padding(.top)
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(NativeSettingsPolicy.catalog.first { $0.id == section }?.title ?? "General").font(.title2.bold())
                Spacer()
                if store.busy.contains("/api/settings") { ProgressView().controlSize(.small) }
                if ["general", "resources", "tailscale", "snapshots", "image"].contains(section) {
                    Text("Changes save automatically").font(.caption).foregroundStyle(.secondary)
                }
            }.padding()
            Divider()
            if store.settings.isNull {
                ContentUnavailableView("Settings unavailable", systemImage: "gearshape",
                                       description: Text("Connect to the AgentVM server to load settings."))
            } else {
                pane.formStyle(.grouped)
            }
        }.frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    @ViewBuilder private var pane: some View {
        switch section {
        case "resources": NativeResourcesPane(store: store)
        case "preferences": NativePreferencesPane()
        case "account": NativeAccountPane(store: store)
        case "git": NativeGitPane(store: store)
        case "tailscale": NativeTailscalePane(store: store)
        case "snapshots": NativeSnapshotSettingsPane(store: store)
        case "backups": NativeBackupSettingsPane(store: store)
        case "storage": NativeStoragePane(store: store)
        case "image": NativeImagePane(store: store)
        default: NativeGeneralPane(store: store)
        }
    }
}
