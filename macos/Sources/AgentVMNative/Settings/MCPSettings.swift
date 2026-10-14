import AppKit
import SwiftUI

struct NativeMCPSettingsPane: View {
    @ObservedObject var store: NativeStore
    private var server: URL? { Bundle.main.resourceURL?.appendingPathComponent("runtime/bin/agentvm-mcp") }
    private var included: Bool { server.map { FileManager.default.isExecutableFile(atPath: $0.path) } ?? false }
    private var configuration: JSON? { server.flatMap { NativeMCPPolicy.configuration(server: $0, endpoint: store.api.baseURL) } }

    var body: some View {
        Form {
            Section("Native MCP server") {
                LabeledContent("Status", value: included ? "Included in this application" : "Not included in this build")
                LabeledContent("Workspace", value: store.api.baseURL.absoluteString)
                NativeSettingsNote(text: "Connect an MCP client to control machines, terminals, files, settings and backups in this workspace.")
            }
            Section("Client configuration") {
                if included, let configuration {
                    Text(configuration.pretty).font(.system(.caption, design: .monospaced)).textSelection(.enabled)
                    Button("Copy configuration") {
                        NSPasteboard.general.clearContents()
                        if NSPasteboard.general.setString(configuration.pretty, forType: .string) {
                            store.notice = "MCP configuration copied."
                        }
                    }.accessibilityIdentifier("settings.mcp.copy")
                    NativeSettingsNote(text: "Use this entry in clients that accept an mcpServers JSON configuration. Provider credentials stay on the host.")
                } else {
                    NativeSettingsNote(text: "Build the latest application with MCP support to include its native server.")
                }
            }
        }.formStyle(.grouped)
    }
}
