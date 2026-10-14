import SwiftUI

struct NativeWorkspace: View {
    @ObservedObject var store: NativeStore
    @Environment(\.scenePhase) private var phase

    private var selection: Binding<NativeRoute?> {
        Binding(get: { store.route }, set: { if let value = $0 { store.route = value } })
    }

    var body: some View {
        NavigationSplitView {
            VStack(spacing: 0) {
                Button { store.launcherPresented = true } label: {
                    Label("New VM", systemImage: "plus").frame(maxWidth: .infinity)
                }.buttonStyle(.borderedProminent).padding()
                List(selection: selection) {
                    Label("Machines", systemImage: "square.grid.2x2").tag(NativeRoute.machines)
                    Label("Snapshots", systemImage: "square.stack.3d.up").tag(NativeRoute.snapshots)
                    Label("Settings", systemImage: "slider.horizontal.3").tag(NativeRoute.settings)
                    Section("Machines") {
                        ForEach(store.tasks.filter { !$0.isEnded }, id: \.id) { task in
                            HStack {
                                Circle().fill(task.isWaiting ? Color.orange : Color.indigo).frame(width: 7, height: 7)
                                Text(task.title).lineLimit(1)
                                Spacer()
                                Text(task.state).font(.caption2).foregroundStyle(.secondary)
                            }.tag(NativeRoute.machine(task.id))
                        }
                    }
                }.listStyle(.sidebar)
                VStack(spacing: 8) {
                    HStack { Text("VMs"); Spacer(); Text("\(store.status["running"].integer) / \(store.status["concurrency"].integer)") }
                    ProgressView(value: Double(store.status["running"].integer),
                                 total: Double(max(1, store.status["concurrency"].integer)))
                    HStack { Text("Memory reserved"); Spacer(); Text(String(format: "%.1f GB", store.status["ram_committed_mb"].number / 1024)) }
                }.font(.caption).foregroundStyle(.secondary).padding()
            }.navigationTitle("agentvm").navigationSplitViewColumnWidth(min: 200, ideal: 230, max: 300)
        } detail: {
            VStack(spacing: 0) {
                if !store.connected {
                    HStack { ProgressView().controlSize(.small); Text("Reconnecting to agentvm…"); Spacer() }
                        .padding(10).background(.orange.opacity(0.12))
                        .help(store.connectionMessage ?? "The engine connection will be restored automatically.")
                }
                Group {
                    switch store.route {
                    case .machines: NativeMachinesView(store: store)
                    case .snapshots: NativeSnapshotsView(store: store)
                    case .settings: NativeSettingsView(store: store)
                    case .machine(let id): NativeMachineView(store: store, id: id).id(id)
                    }
                }.frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .sheet(isPresented: $store.launcherPresented) { NativeLauncherView(store: store) }
        .alert("agentvm", isPresented: Binding(get: { store.error != nil }, set: { if !$0 { store.error = nil } })) {
            Button("OK") { store.error = nil }
        } message: { Text(store.error ?? "") }
        .overlay(alignment: .bottom) {
            if let notice = store.notice {
                Text(notice).padding(12).background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10))
                    .padding().onTapGesture { store.notice = nil }.task(id: notice) {
                        try? await Task.sleep(for: .seconds(5))
                        if store.notice == notice { store.notice = nil }
                    }
            }
        }
        .task {
            #if NATIVE_UI_QA
            Task { await NativeQA.runIfRequested(store: store) }
            #endif
            let attention = NativeAttention()
            while !Task.isCancelled {
                await store.refresh()
                attention.update(store.tasks)
                do { try await Task.sleep(for: .seconds(NativePollingPolicy.overviewSeconds(active: phase == .active))) } catch { break }
            }
        }
        .onChange(of: phase) { _, value in
            if value == .active { Task { await store.refresh() } }
        }
    }
}
