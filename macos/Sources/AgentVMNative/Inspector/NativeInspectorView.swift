import SwiftUI

struct NativeInspectorView: View {
    @ObservedObject var store: NativeStore
    let task: JSON
    @Binding var tab: String

    var body: some View {
        VStack(spacing: 0) {
            Picker("Machine details", selection: $tab) {
                Text("Telemetry").tag("telemetry")
                Text(NativeAgentPresentation.showsClaudeAnalytics(task) ? "Claude" : "Output").tag("claude")
                Text("Diagnostics").tag("diagnostics")
            }.pickerStyle(.segmented).labelsHidden().accessibilityLabel("Machine details")
                .controlSize(.small).padding(10)
            Divider()
            switch tab {
            case "claude":
                if NativeAgentPresentation.showsClaudeAnalytics(task) {
                    NativeConversationView(store: store, task: task)
                } else {
                    NativeAgentOutputView(store: store, task: task)
                }
            case "diagnostics": NativeDiagnosticsView(store: store, task: task)
            default: NativeTelemetryView(store: store, task: task)
            }
        }
    }
}
