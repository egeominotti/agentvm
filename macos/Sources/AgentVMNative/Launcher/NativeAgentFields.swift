import SwiftUI

struct NativeAgentFields: View {
    let catalog: JSON
    let supported: Bool
    let releases: [String]
    @Binding var selection: JSON
    @Binding var version: String
    @Binding var custom: JSON
    @Binding var customVersion: Bool
    @Binding var rememberedProviders: [String: String]
    let configureProviders: () -> Void
    let configureAccounts: () -> Void

    private var kind: String { supported ? NativeAgentPolicy.kind(selection) : "claude" }
    private var provider: String { NativeAgentAuthentication.provider(selection: selection, catalog: catalog) }
    private var modernAuthentication: Bool { supported && NativeAgentAuthentication.supported(catalog: catalog) }
    private var authentication: String { NativeAgentAuthentication.auth(selection: selection, catalog: catalog) }
    private var accounts: [JSON] { NativeAgentAuthentication.accountChoices(selection: selection, catalog: catalog) }

    var kindSelection: Binding<String> { Binding(get: { kind }, set: changeKind) }
    var authenticationSelection: Binding<String> { Binding(get: { authentication }, set: changeAuthentication) }

    var body: some View {
        if supported {
            Picker("Coding CLI", selection: kindSelection) {
                ForEach(Array(NativeAgentPolicy.choices(catalog: catalog).enumerated()), id: \.offset) {
                    Text($0.element["name"].text).tag($0.element["kind"].text)
                }
            }.accessibilityIdentifier("launcher.agent")
            if modernAuthentication {
                Picker("Authentication", selection: authenticationSelection) {
                    Text("Account or subscription").tag("account").disabled(accounts.isEmpty)
                    Text(kind == "qwen" ? "API key / Coding Plan" : "API provider").tag("api")
                }.accessibilityIdentifier("launcher.authentication")
            }
            if modernAuthentication && authentication == "account" {
                let selected = NativeAgentAuthentication.accountKind(selection: selection, catalog: catalog)
                Picker("Account", selection: Binding(get: { selected }, set: {
                    selection = NativeAgentAuthentication.changingAccount(selection: selection, to: $0)
                })) {
                    ForEach(accounts, id: \.id) { account in
                        Text(account["name"].text + (account["connected"].flag ? "" : " (not connected)")).tag(account["id"].text)
                    }
                    if !accounts.contains(where: { $0["id"].text == selected }) {
                        Text("\(selected) (unavailable)").tag(selected).disabled(true)
                    }
                }.accessibilityIdentifier("launcher.account")
                Button("Configure accounts…", action: configureAccounts)
            } else {
                Picker("Inference provider", selection: Binding(get: { provider }, set: {
                    selection["provider"] = $0.isEmpty ? .null : .string($0)
                    rememberedProviders[kind] = $0
                })) {
                    if kind == "claude" && !modernAuthentication { Text("Claude subscription").tag("") }
                    if kind == "custom" { Text("No provider").tag("") }
                    ForEach(NativeAgentPolicy.providerChoices(selection: selection, catalog: catalog)) { choice in
                        Text(choice.available ? choice.name : "\(choice.name) (unavailable)").tag(choice.id).disabled(!choice.available)
                    }
                }
                .accessibilityIdentifier("launcher.provider")
                Button("Configure providers…", action: configureProviders)
            }
        }
        if kind != "custom" {
            Picker(kind == "claude" ? "Claude Code version" : "CLI version", selection: Binding(get: { customVersion ? "__custom" : version }, set: {
                customVersion = $0 == "__custom"
                if !customVersion { version = $0 }
            })) {
                Text(kind == "claude" ? "Image version" : "CLI default").tag("")
                Text("Latest").tag("latest")
                if kind == "claude" {
                    Text("Stable").tag("stable")
                    ForEach(releases, id: \.self) { Text($0).tag($0) }
                }
                Text("Exact version…").tag("__custom")
            }.accessibilityIdentifier("launcher.version")
            if customVersion { TextField("Exact version", text: $version, prompt: Text("1.2.3")).accessibilityIdentifier("launcher.versionID") }
        }
        if kind == "custom" { NativeCustomLaunchFields(custom: $custom) }
    }

    private func changeKind(_ next: String) {
        guard next != kind else { return }
        rememberProvider()
        selection = NativeAgentAuthentication.changingKind(selection: selection, to: next, catalog: catalog)
        version = ""
        customVersion = false
    }

    private func changeAuthentication(_ next: String) {
        rememberProvider()
        selection = NativeAgentAuthentication.changingAuth(selection: selection, to: next, catalog: catalog, rememberedProvider: rememberedProviders[kind])
    }

    private func rememberProvider() {
        if authentication == "api" { rememberedProviders[kind] = provider }
    }
}

private struct NativeCustomLaunchFields: View {
    @Binding var custom: JSON

    var body: some View {
        TextField("CLI name", text: field("name")).accessibilityIdentifier("launcher.custom.name")
        VStack(alignment: .leading, spacing: 5) {
            Text("Installation command (optional)").font(.caption)
            TextEditor(text: field("install")).font(.system(.caption, design: .monospaced)).frame(minHeight: 50)
                .accessibilityIdentifier("launcher.custom.install")
            Text("The installation command runs as shell code inside this VM.").font(.caption).foregroundStyle(.secondary)
        }
        TextField("Interactive command", text: field("interactive"), prompt: Text("my-cli --model {model}"))
            .accessibilityIdentifier("launcher.custom.interactive")
        TextField("Headless command (optional)", text: field("headless"), prompt: Text("my-cli run --prompt {prompt}"))
            .accessibilityIdentifier("launcher.custom.headless")
        TextField("Resume command (optional)", text: field("resume"))
            .accessibilityIdentifier("launcher.custom.resume")
        Text("Launch commands accept arguments and {prompt}, {model}, {endpoint} placeholders. Quotes preserve literal arguments.")
            .font(.caption).foregroundStyle(.secondary)
    }

    private func field(_ key: String) -> Binding<String> {
        Binding(get: { custom[key].text }, set: { custom[key] = $0.isEmpty && !["name", "interactive"].contains(key) ? .null : .string($0) })
    }
}
