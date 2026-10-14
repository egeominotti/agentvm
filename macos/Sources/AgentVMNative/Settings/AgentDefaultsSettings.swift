import SwiftUI

struct NativeAgentDefaultsFields: View {
    @ObservedObject var store: NativeStore
    @State private var selection: JSON
    @State private var version: String
    @State private var rememberedProvider: String?
    private var kind: String { NativeAgentPolicy.kind(selection) }
    private var supported: Bool { store.agentCatalogState == .supported }
    private var choices: [JSON] { NativeAgentPolicy.choices(catalog: store.agentCatalog) }
    private var accountsSupported: Bool { NativeAgentAuthentication.supported(catalog: store.agentCatalog) }
    private var auth: String { NativeAgentAuthentication.auth(selection: selection, catalog: store.agentCatalog) }
    private var accounts: [JSON] {
        NativeAgentAuthentication.accountChoices(selection: selection, catalog: store.agentCatalog)
    }
    private var accountKind: String {
        NativeAgentAuthentication.accountKind(selection: selection, catalog: store.agentCatalog)
    }
    private var providers: [NativeProviderChoice] {
        NativeAgentPolicy.providerChoices(selection: selection, catalog: store.agentCatalog)
    }
    private var busy: Bool { store.busy.contains("/api/settings") }
    private var custom: JSON { NativeAgentSettingsPolicy.reusableCustom(settings: store.settings) }
    private var customReady: Bool { NativeAgentPolicy.customBlocker(custom, interactive: true) == nil }
    private var blocker: String? {
        var preview = selection
        preview["version"] = .string(version.trimmingCharacters(in: .whitespacesAndNewlines))
        if kind == "custom" { preview["custom"] = custom }
        return NativeAgentPolicy.configurationBlocker(selection: preview, catalog: store.agentCatalog)
    }

    @MainActor init(store: NativeStore) {
        self.store = store
        let agent = store.settings["agent"]
        _selection = State(initialValue: agent.isNull ? .object(["kind": .string("claude")]) : agent)
        _version = State(initialValue: agent["version"].text)
        _rememberedProvider = State(initialValue: agent["provider"].text.isEmpty ? nil : agent["provider"].text)
    }

    var body: some View {
        NativeAgentCatalogStatus(store: store)
        Picker("Default coding CLI", selection: Binding(get: { kind }, set: changeKind)) {
            ForEach(Array(choices.enumerated()), id: \.offset) { _, choice in
                Text(choice["name"].text).tag(choice["kind"].text)
                    .disabled(choice["kind"].text == "custom" && !customReady)
            }
            if !choices.contains(where: { $0["kind"].text == kind }) {
                Text("\(kind.capitalized) (unavailable)").tag(kind).disabled(true)
            }
        }.disabled(!supported || busy)
        authenticationFields
        TextField("CLI version (empty uses its default)", text: $version)
            .textFieldStyle(.roundedBorder).onSubmit(save).disabled(!supported || busy)
        NativeSettingsNote(text: "Use latest or an exact version such as 1.2.3; stable is available for Claude Code. Save these defaults for new VMs.")
        NativeSettingsError(message: blocker)
        Button("Save agent defaults", action: save).disabled(!supported || busy || blocker != nil)
        if !customReady {
            Button("Configure a custom CLI…") { UserDefaults.standard.set("custom_cli", forKey: "native.settingsSection") }
        }
        Button("Configure provider endpoints and credentials…") {
            UserDefaults.standard.set("providers", forKey: "native.settingsSection")
        }
    }

    private func changeKind(_ value: String) {
        guard supported, value != kind, value != "custom" || customReady else { return }
        selection = NativeAgentAuthentication.changingKind(selection: selection, to: value, catalog: store.agentCatalog)
        rememberedProvider = selection["provider"].text.isEmpty ? nil : selection["provider"].text
        version = ""
    }

    @ViewBuilder private var authenticationFields: some View {
        if accountsSupported {
            Picker("Authentication", selection: Binding(get: { auth }, set: changeAuth)) {
                Text("Account access").tag("account").disabled(accounts.isEmpty)
                Text("Provider API").tag("api")
                if !["account", "api"].contains(auth) { Text("\(auth) (unavailable)").tag(auth).disabled(true) }
            }.disabled(!supported || busy)
        }
        if accountsSupported && auth == "account" {
            Picker("Default account", selection: Binding(get: { accountKind }, set: {
                selection = NativeAgentAuthentication.changingAccount(selection: selection, to: $0)
            })) {
                ForEach(accounts, id: \.id) { account in
                    Text("\(account["name"].text) · \(account["connected"].flag ? "Connected" : "Not connected")").tag(account.id)
                }
                if !accounts.contains(where: { $0.id == accountKind }) {
                    Text("\(accountKind) (unavailable)").tag(accountKind).disabled(true)
                }
            }.disabled(!supported || busy)
            NativeSettingsNote(text: "Account access uses the selected service's account or plan. Connect it before launching a VM; provider API credentials are a separate choice.")
            Button("Connect agent accounts…") { UserDefaults.standard.set("accounts", forKey: "native.settingsSection") }
        } else {
            Picker("Default provider", selection: Binding(get: {
                NativeAgentAuthentication.provider(selection: selection, catalog: store.agentCatalog)
            }, set: {
                selection["provider"] = $0.isEmpty ? .null : .string($0)
                rememberedProvider = $0.isEmpty ? nil : $0
            })) {
                if !accountsSupported && kind == "claude" { Text("Claude subscription").tag("") }
                if kind == "custom" { Text("No provider").tag("") }
                ForEach(providers) { provider in
                    Text(provider.available ? provider.name : "\(provider.name) (incompatible)")
                        .tag(provider.id).disabled(!provider.available)
                }
            }.disabled(!supported || busy)
        }
    }

    private func changeAuth(_ value: String) {
        guard supported, value != auth else { return }
        if auth == "api" { rememberedProvider = selection["provider"].text.isEmpty ? nil : selection["provider"].text }
        selection = NativeAgentAuthentication.changingAuth(selection: selection, to: value, catalog: store.agentCatalog,
                                                          rememberedProvider: rememberedProvider)
    }

    private func save() {
        guard supported, !busy, blocker == nil else { return }
        let saved = accountsSupported ? NativeAgentAuthentication.changingAuth(selection: selection, to: auth,
                                                                               catalog: store.agentCatalog) : selection
        store.queueSettings(NativeAgentSettingsPolicy.defaultsUpdate(settings: store.settings, selection: saved,
                                                                    version: version))
    }
}
