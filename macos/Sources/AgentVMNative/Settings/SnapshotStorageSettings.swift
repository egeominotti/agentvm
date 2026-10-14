import SwiftUI

struct NativeSnapshotSettingsPane: View {
    @ObservedObject var store: NativeStore
    var body: some View {
        Form {
            Section("Automatic snapshots") {
                Picker("Snapshot running VMs", selection: store.nativeInteger(["auto_snapshots", "every_min"])) {
                    ForEach(NativeSettingsPolicy.intervals, id: \.self) { interval in
                        Text(NativeSettingsPolicy.intervalLabel(interval)).tag(interval)
                    }
                }
                Stepper("Keep the latest \(store.settings["auto_snapshots"]["keep"].integer) per VM",
                        value: store.nativeInteger(["auto_snapshots", "keep"]), in: 1...50)
                Toggle("Take a snapshot before closing", isOn: store.nativeFlag(["auto_snapshots", "before_close"]))
                NativeSettingsNote(text: "Older automatic snapshots are pruned. Manual and imported snapshots stay until you delete them. A machine can override its interval.")
                NativeSettingsError(message: store.error)
            }
        }
    }
}

struct NativeStoragePane: View {
    @ObservedObject var store: NativeStore
    @State private var confirmCleanup = false

    var body: some View {
        Form {
            Section("On this Mac") {
                ForEach([( "vm_disks_mb", "Running VM disks"), ("snapshots_mb", "Snapshots"),
                         ("golden_mb", "VM image"), ("jobs_mb", "Logs and Claude history")], id: \.0) { part in
                    LabeledContent(part.1, value: NativeSettingsPolicy.size(store.storage[part.0].integer))
                }
                LabeledContent("Machine folders", value: "\(store.storage["jobs"].integer)")
                NativeSettingsNote(text: "Closed VM disks are deleted. Interrupted disks remain in Snapshots; logs and Claude history stay until removed.")
                Button("Refresh disk usage") { Task { await load() } }
            }
            Section("Delete logs of closed machines") {
                NativeSettingsNote(text: "This permanently removes closed machines' diagnostics and Claude history.")
                Button("Delete closed machine logs…", role: .destructive) { confirmCleanup = true }
                    .disabled(store.busy.contains("/api/storage/cleanup"))
                NativeSettingsError(message: store.error)
            }
        }
        .task { await load() }
        .confirmationDialog("Delete closed machine logs and history permanently?", isPresented: $confirmCleanup) {
            Button("Delete logs", role: .destructive) {
                Task {
                    if let result = await store.perform("/api/storage/cleanup") {
                        store.notice = "Deleted \(result["removed"].integer) machine folders"
                    }
                    await load()
                }
            }
        }
    }

    private func load() async {
        do { store.storage = try await store.api.request("/api/storage") }
        catch is CancellationError {}
        catch { if !Task.isCancelled { store.error = error.localizedDescription } }
    }
}
