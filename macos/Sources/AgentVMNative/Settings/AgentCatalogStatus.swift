import SwiftUI

struct NativeAgentCatalogStatus: View {
    @ObservedObject var store: NativeStore

    @ViewBuilder var body: some View {
        switch store.agentCatalogState {
        case .loading:
            ProgressView("Reading coding CLI settings…")
        case .supported:
            EmptyView()
        case .legacy:
            NativeSettingsNote(text: "This engine supports Claude Code. Multi-CLI settings require a newer AgentVM engine.")
        case .failed(let message):
            NativeSettingsError(message: message)
            Button("Retry coding CLI settings", systemImage: "arrow.clockwise") { Task { await store.refreshAgents() } }
        }
    }
}
