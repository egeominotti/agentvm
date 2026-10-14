import SwiftUI

struct NativeProvidersPane: View {
    @ObservedObject var store: NativeStore
    @State private var selected = "openai"
    @State private var drafts: [String: JSON] = [:]
    private var providers: [JSON] { store.agentCatalog["providers"].items }
    private var provider: JSON { providers.first { $0.id == selected } ?? providers.first ?? .null }

    var body: some View {
        Form {
            Section("Inference providers") {
                NativeAgentCatalogStatus(store: store)
                if !providers.isEmpty {
                    Picker("Provider", selection: $selected) {
                        ForEach(providers, id: \.id) { Text($0["name"].text).tag($0.id) }
                    }
                    NativeProviderEditor(store: store, provider: provider, draft: Binding(
                        get: { drafts[provider.id] ?? provider["config"] },
                        set: { drafts[provider.id] = $0 }
                    )).id(provider.id)
                }
            }
        }
        .task {
            await store.refreshAgents()
            if !providers.contains(where: { $0.id == selected }), let first = providers.first { selected = first.id }
        }
    }
}

private struct NativeProviderEditor: View {
    @ObservedObject var store: NativeStore
    let provider: JSON
    @Binding var draft: JSON
    private var ready: Bool { store.agentCatalogState == .supported }
    private var path: String { "/api/settings/providers/\(provider.id)" }
    private var blocker: String? {
        NativeAgentSettingsPolicy.providerBlocker(endpoint: draft["endpoint"].text, protocolID: draft["protocol"].text)
    }

    var body: some View {
        Text("API endpoint").font(.callout)
        TextField("https://api.example.com/v1", text: field("endpoint"))
            .textFieldStyle(.roundedBorder).labelsHidden().accessibilityLabel("API endpoint")
        Picker("API protocol", selection: field("protocol")) {
            Text("OpenAI").tag("openai")
            Text("Anthropic").tag("anthropic")
            Text("Google Gemini").tag("google")
            if !["openai", "anthropic", "google"].contains(draft["protocol"].text) {
                Text(draft["protocol"].text).tag(draft["protocol"].text)
            }
        }
        Toggle("Require an API key", isOn: Binding(get: { draft["requires_key"].flag },
                                                   set: { draft["requires_key"] = .bool($0) }))
        NativeSettingsNote(text: "New VMs use this endpoint. Provider credentials stay on this Mac and reach the provider through the host relay.")
        NativeSettingsError(message: draft.isNull ? nil : blocker)
        Button("Save provider configuration") { Task { _ = await save() } }
            .disabled(!ready || draft.isNull || blocker != nil || store.busy.contains(path))
        Button("Discard configuration edits") { draft = provider["config"] }
            .disabled(draft == provider["config"] || store.busy.contains(path))
        Divider()
        NativeSecretField(label: "\(provider["name"].text) API key", placeholder: "Paste an API key",
                          saved: provider["has_key"].flag, savedMessage: "API credential available on this Mac",
                          saveEnabled: !draft.isNull && blocker == nil,
                          save: { await save(token: $0) }, remove: {
            await store.perform(path, method: "DELETE", success: "Provider Keychain credential removed") != nil
        }).disabled(!ready || store.busy.contains(path))
        NativeSettingsNote(text: "An empty key keeps the existing credential. Remove deletes the Keychain copy; a key supplied in the engine's environment remains available.")
        NativeSettingsError(message: store.error)
    }

    private func field(_ key: String) -> Binding<String> {
        Binding(get: { draft[key].text }, set: { draft[key] = .string($0) })
    }

    private func save(token: String? = nil) async -> Bool {
        guard ready, blocker == nil, !draft.isNull else { return false }
        let sent = draft
        let result = await store.perform(path, method: "PUT",
                                         body: NativeAgentSettingsPolicy.providerUpdate(config: sent, token: token),
                                         success: token == nil ? "Provider configuration saved" : "Provider API key saved in the Keychain")
        if result != nil, draft == sent,
           let saved = store.agentCatalog["providers"].items.first(where: { $0.id == provider.id }) {
            draft = saved["config"]
        }
        return result != nil
    }
}
