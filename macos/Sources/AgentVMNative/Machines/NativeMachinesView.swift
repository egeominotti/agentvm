import SwiftUI

struct NativeMachinesView: View {
    @ObservedObject var store: NativeStore
    @AppStorage("native.machineLayout") private var layout = "grid"
    @State private var query = ""
    @State private var section = "live"
    @State private var filter = "all"
    @State private var newest = false
    @State private var selected = Set<String>()
    @State private var deletionIDs: [String] = []
    @State private var confirmDelete = false

    private var shown: [JSON] {
        NativeMachineLogic.sorted(store.tasks, newest: newest).filter { task in
            let ended = NativeMachineLogic.ended(task)
            let sectionMatches = section == "all" || (section == "finished" ? ended : !ended)
            let category = ended ? NativeMachineLogic.state(task) : NativeMachineLogic.category(task)
            return sectionMatches && (filter == "all" || category == filter) && NativeMachineLogic.matches(task, query: query)
        }
    }

    private var selectedShown: [String] {
        shown.filter { NativeMachineLogic.ended($0) && selected.contains($0["id"].text) }.map { $0["id"].text }
    }

    var body: some View {
        VStack(spacing: 18) {
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Machines").font(.largeTitle.bold())
                    Text("Disposable workspaces. Your repositories stay on your Mac.").foregroundStyle(.secondary)
                }
                Spacer()
                Button("New VM", systemImage: "plus") { store.launcherPresented = true }.buttonStyle(.borderedProminent)
            }
            hostResources
            HStack {
                TextField("Search title, repository, branch or id", text: $query).textFieldStyle(.roundedBorder)
                Picker("Machines", selection: $section) {
                    Text("Live").tag("live")
                    Text("Finished").tag("finished")
                    Text("All").tag("all")
                }.labelsHidden().accessibilityLabel("Machines to show").frame(width: 130).fixedSize(horizontal: true, vertical: false)
                Picker("Layout", selection: $layout) {
                    Label("Grid", systemImage: "square.grid.2x2").labelStyle(.iconOnly).tag("grid")
                    Label("List", systemImage: "list.bullet").labelStyle(.iconOnly).tag("list")
                }.pickerStyle(.segmented).labelsHidden().accessibilityLabel("Layout")
                    .frame(width: 80).fixedSize(horizontal: true, vertical: false)
            }
            HStack {
                Picker("Activity or outcome", selection: $filter) {
                    Text("All states").tag("all")
                    if section != "finished" {
                        Text("Waiting for you").tag("waiting")
                        Text("Working").tag("working")
                        Text("Starting").tag("starting")
                        Text("Ready").tag("ready")
                    }
                    if section != "live" {
                        Text("Done").tag("done")
                        Text("No changes").tag("no_changes")
                        Text("Failed").tag("failed")
                        Text("Stopped").tag("stopped")
                    }
                }.labelsHidden().accessibilityLabel("Activity or outcome")
                    .frame(width: 180).fixedSize(horizontal: true, vertical: false)
                Picker("Sort", selection: $newest) {
                    Text("Oldest first").tag(false)
                    Text("Newest first").tag(true)
                }.labelsHidden().accessibilityLabel("Sort order").frame(width: 150).fixedSize(horizontal: true, vertical: false)
                Spacer()
            }
            if !selectedShown.isEmpty {
                HStack {
                    Text("\(selectedShown.count) finished machines selected")
                    Button("Clear") { selected.removeAll() }
                    Spacer()
                    Button("Delete logs", role: .destructive) {
                        deletionIDs = selectedShown
                        confirmDelete = true
                    }
                }.padding(10).background(.quaternary, in: RoundedRectangle(cornerRadius: 8))
            }
            ScrollView {
                if shown.isEmpty {
                    ContentUnavailableView(
                        store.connected ? "No machines here" : "Connecting to agentvm",
                        systemImage: "desktopcomputer",
                        description: Text(query.isEmpty ? "Launch a VM to get started." : "No machine matches your search.")
                    ).frame(maxWidth: .infinity, minHeight: 240)
                } else if layout == "grid" {
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 310), spacing: 16)], spacing: 16) {
                        ForEach(shown, id: \.["id"].text) { task in
                            NativeMachineCard(store: store, task: task, selected: selection(task))
                        }
                    }
                } else {
                    LazyVStack(spacing: 8) {
                        ForEach(shown, id: \.["id"].text) { task in
                            NativeMachineRow(store: store, task: task, selected: selection(task))
                        }
                    }
                }
            }
        }
        .padding(22)
        .background(NativePreviewRuntimeView().frame(width: 1, height: 1).opacity(0)
            .allowsHitTesting(false).accessibilityHidden(true))
        .onChange(of: section) { _, _ in filter = "all" }
        .confirmationDialog("Delete \(deletionIDs.count) machines with their logs and Claude history? Their branches stay.", isPresented: $confirmDelete) {
            Button("Delete logs permanently", role: .destructive) {
                let ids = deletionIDs
                Task {
                    let completed = await store.performMachineDeletion(ids: ids)
                    selected.subtract(completed)
                }
            }
        }
    }

    private var hostResources: some View {
        HStack(spacing: 28) {
            Label("\(store.status["running"].integer) / \(store.status["concurrency"].integer) VMs running", systemImage: "desktopcomputer")
            Label("\(store.status["host"]["cpus"].integer) host cores", systemImage: "cpu")
            VStack(alignment: .leading, spacing: 4) {
                Text("\(MachineFormat.memory(store.status["ram_committed_mb"].number)) committed / \(MachineFormat.memory(store.status["host"]["ram_mb"].number)) RAM")
                ProgressView(value: store.status["ram_committed_mb"].number, total: max(1, store.status["host"]["ram_mb"].number))
            }.frame(maxWidth: 320)
            Spacer()
        }.font(.callout).padding(14).background(.quaternary, in: RoundedRectangle(cornerRadius: 12))
    }

    private func selection(_ task: JSON) -> Binding<Bool> {
        let id = task["id"].text
        return Binding(get: { selected.contains(id) }, set: { value in
            if value { selected.insert(id) } else { selected.remove(id) }
        })
    }
}
