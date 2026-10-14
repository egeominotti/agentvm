import SwiftUI

struct NativeDiagnosticsView: View {
    @ObservedObject var store: NativeStore
    let task: JSON
    @State private var diagnostics: JSON = .null
    @State private var error: String?
    @State private var attempt = 0
    @State private var expanded = Set<String>()

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                HStack {
                    Button("Refresh", systemImage: "arrow.clockwise") { attempt += 1 }
                    Spacer()
                    Button("Copy report", systemImage: "doc.on.doc") {
                        MachineFormat.copy(NativeInspectorLogic.report(id: task["id"].text, diagnostics: diagnostics))
                        store.notice = "Diagnostics copied"
                    }.disabled(diagnostics.isNull)
                }
                if let error {
                    Text("Could not read diagnostics: \(error)").foregroundStyle(.red)
                    Button("Try again") { attempt += 1 }
                } else if diagnostics.isNull { ProgressView("Reading logs…") }
                else {
                    Text(diagnostics["summary"].text).font(.headline).textSelection(.enabled)
                    if !diagnostics["hint"].text.isEmpty { Text(diagnostics["hint"].text).foregroundStyle(.secondary).textSelection(.enabled) }
                    ForEach(Array(diagnostics["timeline"].items.enumerated()), id: \.offset) { index, step in
                        HStack {
                            Text(Date(timeIntervalSince1970: step["at"].number).formatted(date: .omitted, time: .standard))
                                .foregroundStyle(.secondary)
                            Text(step["state"].text.replacingOccurrences(of: "_", with: " ")).bold()
                            Spacer()
                            if index > 0 {
                                Text("+\(MachineFormat.duration(step["at"].number - diagnostics["timeline"].items[index - 1]["at"].number))")
                                    .foregroundStyle(.secondary)
                            }
                        }.font(.caption)
                    }
                    ForEach(diagnostics["logs"].items, id: \.["file"].text) { log in
                        DisclosureGroup("\(log["name"].text) · \(log["file"].text)", isExpanded: expansion(log["file"].text)) {
                            Text(log["tail"].text.isEmpty ? "(empty)" : log["tail"].text)
                                .font(.system(.caption, design: .monospaced)).textSelection(.enabled)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }.padding(10).background(Color.secondary.opacity(0.06), in: RoundedRectangle(cornerRadius: 8))
                    }
                    if !diagnostics["server_log"].items.isEmpty {
                        DisclosureGroup("Server log") {
                            Text(diagnostics["server_log"].items.map(\.text).joined(separator: "\n"))
                                .font(.system(.caption, design: .monospaced)).textSelection(.enabled)
                        }
                    }
                    if diagnostics["logs"].items.isEmpty {
                        Text(NativeMachineLogic.ended(task) ? "No logs remain. They were not written or were deleted in Settings → Storage." : "No logs yet.")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
            }.padding(14)
        }
        .task(id: "\(task["id"].text)/\(attempt)") {
            do {
                let result = try await store.api.request("/api/tasks/\(task["id"].text)/diagnostics")
                guard !Task.isCancelled else { return }
                if diagnostics.isNull, let first = NativeInspectorLogic.firstLog(result) { expanded.insert(first) }
                diagnostics = result
                error = nil
            }
            catch is CancellationError { }
            catch { if !Task.isCancelled { self.error = error.localizedDescription } }
        }
    }

    private func expansion(_ file: String) -> Binding<Bool> {
        Binding(get: { expanded.contains(file) }, set: { value in
            if value { expanded.insert(file) } else { expanded.remove(file) }
        })
    }
}
