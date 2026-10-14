import SwiftUI

struct NativeMachineOutcome: View {
    @ObservedObject var store: NativeStore
    let task: JSON
    @State private var files: [NativeDiffFile] = []
    @State private var diffError: String?
    private var base: String { "/api/tasks/\(task["id"].text)" }
    private var kept: JSON? {
        store.snapshots.first { $0["source_task"].text == task["id"].text && $0["name"].text.hasPrefix("Interrupted: ") }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                switch NativeMachineLogic.state(task) {
                case "done":
                    Text("\(task["status"]["commits"].integer) \(task["status"]["commits"].integer == 1 ? "commit" : "commits") on \(task["status"]["branch"].text)").font(.title2.bold())
                    Text("Already in your repository. To try or merge the work:")
                    command("git -C \(NativeMachineLogic.shellQuote(task["repo"].text)) switch \(NativeMachineLogic.shellQuote(task["branch"].text))")
                    command("git -C \(NativeMachineLogic.shellQuote(task["repo"].text)) merge \(NativeMachineLogic.shellQuote(task["branch"].text))")
                case "no_changes":
                    Text("Closed without changes").font(.title2.bold())
                    Text("Nothing was committed, so no branch was created.")
                case "stopped":
                    Text("Force stopped").font(.title2.bold())
                    Text("What was saved is on \(task["branch"].text). Unsaved work is on its kept disk in Snapshots.")
                case "failed":
                    Text("Machine failed").font(.title2.bold())
                    Text("Diagnostics show what happened.")
                    DisclosureGroup("What the server reported") {
                        Text(task["status"]["reason"].text).font(.system(.caption, design: .monospaced)).textSelection(.enabled)
                    }
                default: EmptyView()
                }
                if let kept {
                    GroupBox("Its disk was kept with unsaved work and agent history") {
                        Button("Resume in a new VM", systemImage: "arrow.counterclockwise") {
                            Task {
                                if let created = await store.restoreSnapshot(kept.id), !created["id"].text.isEmpty {
                                    store.route = .machine(created["id"].text)
                                }
                            }
                        }.buttonStyle(.borderedProminent).padding(8)
                            .disabled(store.busy.contains("/api/snapshots/\(kept.id)/restore"))
                    }
                }
                if let diffError {
                    Text("Could not read the changes: \(diffError)").foregroundStyle(.red)
                    Button("Try again") { Task { await loadDiff() } }
                }
                if !files.isEmpty {
                    Text("\(files.count) files changed · +\(files.reduce(0) { $0 + $1.added }) −\(files.reduce(0) { $0 + $1.removed })")
                        .font(.headline)
                    ForEach(files) { file in
                        DisclosureGroup("\(file.path)    +\(file.added) −\(file.removed)") {
                            VStack(alignment: .leading, spacing: 0) {
                                ForEach(Array(file.lines.enumerated()), id: \.offset) { _, line in
                                    Text(line.isEmpty ? " " : line)
                                        .font(.system(.caption, design: .monospaced))
                                        .foregroundStyle(line.hasPrefix("+") ? Color.green : line.hasPrefix("-") ? .red : .primary)
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                }
                            }.textSelection(.enabled).padding(8)
                        }.padding(10).background(Color.secondary.opacity(0.07), in: RoundedRectangle(cornerRadius: 8))
                    }
                }
            }.padding(24).frame(maxWidth: .infinity, alignment: .leading)
        }
        .task(id: task["id"].text) {
            await store.refreshSnapshots()
            if NativeMachineLogic.state(task) == "done" { await loadDiff() }
        }
    }

    private func command(_ value: String) -> some View {
        HStack {
            Text(value).font(.system(.callout, design: .monospaced)).textSelection(.enabled)
            Spacer()
            Button("Copy", systemImage: "doc.on.doc") { MachineFormat.copy(value); store.notice = "Copied the command" }
        }.padding(10).background(Color.secondary.opacity(0.07), in: RoundedRectangle(cornerRadius: 8))
    }

    private func loadDiff() async {
        do { files = NativeMachineLogic.diff(try await store.api.text("\(base)/diff")); diffError = nil }
        catch is CancellationError { }
        catch { if !Task.isCancelled { diffError = error.localizedDescription } }
    }
}
