import SwiftUI

struct NativeCustomCLIPane: View {
    @ObservedObject var store: NativeStore
    @State private var name = ""
    @State private var install = ""
    @State private var interactive = ""
    @State private var headless = ""
    @State private var resume = ""
    @State private var saving = false
    @State private var removePresented = false

    private var supported: Bool { store.agentCatalogState == .supported }
    private var value: JSON {
        NativeAgentSettingsPolicy.customValue(name: name, install: install, interactive: interactive,
                                             headless: headless, resume: resume)
    }
    private var blocker: String? { NativeAgentPolicy.customBlocker(value, interactive: true) }
    private var isDefault: Bool { NativeAgentPolicy.kind(store.settings["agent"]) == "custom" }

    var body: some View {
        Form {
            Section { NativeAgentCatalogStatus(store: store) }
            Section("Reusable custom CLI") {
                TextField("Name", text: $name)
                command("Interactive command", placeholder: "my-cli --model '{model}'", text: $interactive)
                command("Automatic command (optional)", placeholder: "my-cli run --prompt '{prompt}'", text: $headless)
                command("Resume command (optional)", placeholder: "my-cli resume", text: $resume)
                NativeSettingsNote(text: "Launch commands are parsed into arguments. Use {prompt}, {model} and {endpoint} inside arguments; quoting and placeholders are preserved. An automatic task needs an automatic command.")
                if !name.isEmpty || !interactive.isEmpty { NativeSettingsError(message: blocker) }
            }.disabled(!supported || saving)
            Section("Installation (optional)") {
                TextEditor(text: $install).font(.system(.body, design: .monospaced))
                    .frame(minHeight: 100).accessibilityLabel("Installation command")
                NativeSettingsNote(text: "This installation script runs as shell code only when the custom CLI is selected. Keep credentials out of commands; configure a provider for host-managed authentication.")
            }.disabled(!supported || saving)
            Section {
                Button(saving ? "Saving…" : "Save custom CLI") { Task { await save(value) } }
                    .disabled(!supported || saving || blocker != nil)
                if !store.settings["custom_cli"].isNull {
                    Button("Remove saved custom CLI…", role: .destructive) { removePresented = true }
                        .disabled(!supported || saving || isDefault)
                    if isDefault { NativeSettingsNote(text: "Choose another default coding CLI in General before removing this command.") }
                }
                NativeSettingsError(message: store.error)
            }
        }
        .task {
            let custom = NativeAgentSettingsPolicy.reusableCustom(settings: store.settings)
            name = custom["name"].text
            install = custom["install"].text
            interactive = custom["interactive"].text
            headless = custom["headless"].text
            resume = custom["resume"].text
            await store.refreshAgents()
        }
        .confirmationDialog("Remove the saved custom CLI?", isPresented: $removePresented) {
            Button("Remove custom CLI", role: .destructive) { Task { await save(.null) } }
        }
    }

    private func command(_ title: String, placeholder: String, text: Binding<String>) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(title).font(.callout)
            TextField(placeholder, text: text).font(.system(.body, design: .monospaced))
                .textFieldStyle(.roundedBorder).labelsHidden().accessibilityLabel(title)
        }
    }

    private func save(_ custom: JSON) async {
        guard supported, !saving else { return }
        if custom.isNull && isDefault { return }
        if !custom.isNull && NativeAgentPolicy.customBlocker(custom, interactive: true) != nil { return }
        saving = true
        defer { saving = false }
        var next = store.settings
        next["custom_cli"] = custom
        if isDefault { next["agent"]["custom"] = .null }
        await store.saveSettings(next)
    }
}
