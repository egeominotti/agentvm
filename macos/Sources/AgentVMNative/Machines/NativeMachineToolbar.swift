import AppKit
import SwiftUI

struct NativeMachineToolbar: View {
    @ObservedObject var store: NativeStore
    let task: JSON
    @Binding var details: Bool
    @Binding var showFiles: Bool
    let destructive: (String) -> Void
    private var base: String { "/api/tasks/\(task["id"].text)" }
    private var running: Bool { NativeMachineLogic.state(task) == "running" }
    private var lifecycleBusy: Bool { NativeMachineLogic.saveOrCloseBusy(base: base, busy: store.busy) }

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text(NativeMachineLogic.title(task)).font(.headline).lineLimit(1)
                NativeMachineStatus(task: task)
            }
            Text("#\(task["id"].text.suffix(4))").font(.caption.monospaced()).foregroundStyle(.secondary)
            Spacer()
            if task["interactive"].flag && !NativeMachineLogic.ended(task) {
                Button("Save", systemImage: "square.and.arrow.down") {
                    Task {
                        guard !lifecycleBusy else { return }
                        if let result = await store.perform("\(base)/save") {
                            store.notice = result["commits"].integer > 0
                                ? "Saved \(result["commits"].integer) \(result["commits"].integer == 1 ? "commit" : "commits") to \(result["branch"].text)"
                                : "Nothing new to save: no commits yet"
                        }
                    }
                }.disabled(!running || lifecycleBusy).accessibilityIdentifier("machine.save")
                Button("Close", systemImage: "power") {
                    Task {
                        guard !lifecycleBusy else { return }
                        _ = await store.perform("\(base)/close")
                    }
                }.buttonStyle(.borderedProminent).disabled(!running || lifecycleBusy).accessibilityIdentifier("machine.close")
            }
            Menu {
                if !NativeMachineLogic.ended(task) {
                    Button("Take a snapshot now") {
                        Task {
                            _ = await store.perform("\(base)/snapshot", body: .object([:]), success: "Snapshot saved")
                            await store.refreshSnapshots()
                        }
                    }.disabled(!running)
                    Menu("Automatic snapshots") {
                        interval("As in Settings", value: .null)
                        ForEach([0, 5, 15, 30, 60, 120], id: \.self) { minutes in
                            interval(minutes == 0 ? "Off" : "Every \(minutes) min", value: .number(Double(minutes)))
                        }
                    }
                    Button("Copy files into the VM…") { showFiles = true }.disabled(!running || !task["interactive"].flag)
                    Divider()
                }
                Button("Copy branch name") { MachineFormat.copy(task["branch"].text) }
                Button("Push branch to origin") {
                    Task {
                        if let result = await store.perform("\(base)/push", success: "Branch pushed to origin"),
                           !result["pull_request"].text.isEmpty, let url = URL(string: result["pull_request"].text),
                           ["http", "https"].contains(url.scheme ?? "") { NSWorkspace.shared.open(url) }
                    }
                }.disabled(store.busy.contains("\(base)/push"))
                Divider()
                if NativeMachineLogic.ended(task) { Button("Delete this machine", role: .destructive) { destructive("delete") } }
                else { Button("Force stop", role: .destructive) { destructive("stop") } }
            } label: { Image(systemName: "ellipsis.circle") }
                .menuStyle(.borderlessButton).fixedSize()
                .accessibilityLabel("More machine actions").accessibilityIdentifier("machine.actions")
            Button { details.toggle() } label: { Image(systemName: "sidebar.right") }
                .help("Show or hide details (⌘J)").accessibilityLabel(details ? "Hide machine details" : "Show machine details")
                .accessibilityIdentifier("machine.details")
        }.padding(14)
    }

    private func interval(_ title: String, value: JSON) -> some View {
        Button {
            Task { _ = await store.perform("\(base)/auto-snapshots", method: "PUT", body: .object(["every_min": value])) }
        } label: {
            let selected = value.isNull ? task["auto_snapshot_min"].isNull
                : !task["auto_snapshot_min"].isNull && task["auto_snapshot_min"].integer == value.integer
            Label(title, systemImage: selected ? "checkmark" : "circle")
        }
    }
}
