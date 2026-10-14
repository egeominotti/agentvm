import SwiftUI

struct NativeConversationView: View {
    let store: NativeStore
    let task: JSON
    @Environment(\.scenePhase) private var scenePhase
    @State private var entries: [JSON] = []
    @State private var cursor: JSON = .null
    @State private var samples: [JSON] = []
    @State private var history = NativeConversationHistory.empty
    @State private var loaded = false
    @State private var error: String?
    @State private var attempt = 0

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 14) {
                if let last = samples.last { usage(last) }
                NativeMetricChart(title: "Cost at API prices", series: history.cost,
                    timeline: history.usageTimeline, format: MachineFormat.money)
                NativeMetricChart(title: "Output tokens per call", series: history.output,
                    timeline: history.outputTimeline, format: MachineFormat.tokens)
                NativeMetricChart(title: "Context used", series: history.context,
                    timeline: history.usageTimeline, maximum: 100,
                    empty: task["interactive"].flag ? "No data yet" : "Not reported for automatic tasks") { "\(Int($0.rounded()))%" }
                HStack {
                    Text("Conversation").font(.headline)
                    Spacer()
                    Button("Refresh", systemImage: "arrow.clockwise") { attempt += 1 }
                }
                ForEach(Array(entries.enumerated()), id: \.offset) { _, entry in NativeConversationEntry(entry: entry) }
                if let error {
                    Text("Could not read the conversation: \(error)").font(.caption).foregroundStyle(.red)
                    Button("Try again") { attempt += 1 }
                } else if loaded && entries.isEmpty { Text("No conversation yet.").font(.caption).foregroundStyle(.secondary) }
            }.padding(14)
        }
        .task(id: "\(task["id"].text)/\(NativeMachineLogic.ended(task))/\(attempt)/\(scenePhase == .active)") {
            guard scenePhase == .active else { return }
            repeat {
                await read()
                if NativeMachineLogic.ended(task) { break }
                try? await Task.sleep(for: .seconds(5))
            } while !Task.isCancelled
        }
    }

    private func usage(_ sample: JSON) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("\(MachineFormat.money(sample["cost_usd"].number)) at API prices").font(.headline)
            Text("\(MachineFormat.tokens(sample["input_tokens"].number)) tokens in · \(MachineFormat.tokens(sample["output_tokens"].number)) tokens out")
                .font(.caption).foregroundStyle(.secondary)
            if !sample["context_pct"].isNull { Text("Context \(Int(sample["context_pct"].number.rounded()))%").font(.caption) }
        }
    }

    private func read() async {
        do {
            var more = true
            while more && !Task.isCancelled {
                let query = cursor.isNull ? "" : "?cursor=\(try NativeInspectorLogic.cursorQuery(cursor))"
                let page = try await store.api.request("/api/tasks/\(task["id"].text)/claude\(query)")
                guard !Task.isCancelled else { return }
                entries += page["entries"].items
                cursor = page["cursor"]
                more = page["more"].flag
            }
            guard !Task.isCancelled else { return }
            samples = try await store.api.request("/api/tasks/\(task["id"].text)/claude/usage")["samples"].items
            loaded = true
            error = nil
        } catch is CancellationError { }
        catch { if !Task.isCancelled { self.error = error.localizedDescription } }
        guard !Task.isCancelled else { return }
        let currentEntries = entries
        let currentSamples = samples
        let prepared = await Task.detached(priority: .utility) {
            NativeConversationHistory(entries: currentEntries, samples: currentSamples)
        }.value
        if !Task.isCancelled { history = prepared }
    }
}

struct NativeConversationEntry: View {
    let entry: JSON
    private var timestamp: String {
        NativeInspectorLogic.timestamp(entry["at"].text)?.formatted(date: .omitted, time: .shortened) ?? ""
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("\(timestamp) · \(entry["kind"].text.replacingOccurrences(of: "_", with: " "))\(entry["sidechain"].flag ? " · subagent" : "")")
                .font(.caption).foregroundStyle(.secondary)
            switch entry["kind"].text {
            case "tool_use":
                Text(entry["name"].text).font(.caption.bold())
                Text(entry["input"].text).font(.system(.caption, design: .monospaced)).textSelection(.enabled)
            case "tool_result":
                DisclosureGroup(entry["is_error"].flag ? "Error" : "Result") {
                    Text(entry["output"].text).font(.system(.caption, design: .monospaced)).textSelection(.enabled)
                        .foregroundStyle(entry["is_error"].flag ? Color.red : .primary)
                }
            default:
                Text(entry["text"].text).font(.callout).textSelection(.enabled)
            }
        }.padding(12).frame(maxWidth: .infinity, alignment: .leading)
            .background(entry["kind"].text == "user" ? Color.accentColor.opacity(0.1) : Color.secondary.opacity(0.06), in: RoundedRectangle(cornerRadius: 8))
    }
}
