import SwiftUI

struct NativeAgentOutputView: View {
    let task: JSON
    @StateObject private var feed: NativeAgentOutputFeed
    @State private var attempt = 0
    @Environment(\.scenePhase) private var scenePhase

    init(store: NativeStore, task: JSON) {
        self.task = task
        _feed = StateObject(wrappedValue: NativeAgentOutputFeed(api: store.api))
    }

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 12) {
                HStack {
                    Text("Agent output").font(.headline)
                    Spacer()
                    Button("Refresh", systemImage: "arrow.clockwise") { attempt += 1 }
                }
                if !feed.state.isEmpty {
                    Text(feed.state.replacingOccurrences(of: "_", with: " ").capitalized)
                        .font(.caption).foregroundStyle(.secondary)
                }
                ForEach(feed.entries) { entry in
                    VStack(alignment: .leading, spacing: 6) {
                        Text(entry.title).font(.caption.bold())
                        if !entry.text.isEmpty {
                            Text(entry.text).font(.system(.callout, design: .monospaced)).textSelection(.enabled)
                        }
                    }.padding(12).frame(maxWidth: .infinity, alignment: .leading)
                        .foregroundStyle(entry.isError ? Color.red : .primary)
                        .background(Color.secondary.opacity(0.06), in: RoundedRectangle(cornerRadius: 8))
                }
                if let error = feed.error {
                    Text("Could not read the agent output: \(error)").font(.caption).foregroundStyle(.red)
                    Button("Try again") { attempt += 1 }
                } else if !feed.loaded {
                    ProgressView("Reading events…").controlSize(.small)
                } else if feed.entries.isEmpty {
                    Text("No output recorded yet.").font(.caption).foregroundStyle(.secondary)
                }
                if feed.dropped > 0 { Text("Earlier output is omitted.").font(.caption).foregroundStyle(.secondary) }
            }.padding(14)
        }
        .task(id: "\(task.id)/\(NativeMachineLogic.ended(task))/\(scenePhase == .active)/\(attempt)") {
            guard scenePhase == .active else { return }
            await feed.run(id: task.id, ended: NativeMachineLogic.ended(task))
        }
    }
}
