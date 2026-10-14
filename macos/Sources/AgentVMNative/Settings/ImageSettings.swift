import SwiftUI

struct NativeImagePane: View {
    @ObservedObject var store: NativeStore
    @State private var golden: JSON = .null
    @State private var releases: JSON = .null
    @State private var customVersion = ""
    @State private var editingVersion = false

    private var version: Binding<String> {
        Binding(get: { editingVersion ? "custom" : store.settings["claude_version"].text }, set: { value in
            editingVersion = value == "custom"
            if !editingVersion { store.nativeSetSetting(["claude_version"], .string(value)) }
        })
    }

    var body: some View {
        Form {
            Section("Current image") {
                LabeledContent("State", value: golden.isNull ? "Checking…" : golden["rebuilding"].flag ? "Rebuilding" : golden["exists"].flag ? "Ready" : "Missing")
                LabeledContent("Claude Code", value: golden.isNull ? "—" : golden["claude_version"].text.isEmpty ? "Unknown" : golden["claude_version"].text)
                LabeledContent("Size on disk", value: golden.isNull ? "—" : NativeSettingsPolicy.size(golden["size_mb"].integer))
                if golden["built_at"].number > 0 {
                    LabeledContent("Built", value: Date(timeIntervalSince1970: golden["built_at"].number).formatted())
                }
            }
            Section("Next rebuild") {
                Picker("Claude Code version", selection: version) {
                    Text(releases["latest"].text.isEmpty ? "Latest" : "Latest (\(releases["latest"].text))").tag("latest")
                    Text(releases["stable"].text.isEmpty ? "Stable" : "Stable (\(releases["stable"].text))").tag("stable")
                    ForEach(releases["versions"].items.map(\.text), id: \.self) { Text($0).tag($0) }
                    if !["latest", "stable"].contains(store.settings["claude_version"].text),
                       !releases["versions"].items.map(\.text).contains(store.settings["claude_version"].text) {
                        Text(store.settings["claude_version"].text).tag(store.settings["claude_version"].text)
                    }
                    Text("Exact version…").tag("custom")
                }
                if editingVersion {
                    HStack {
                        TextField("Version (for example 2.1.294)", text: $customVersion)
                        Button("Save version") {
                            let version = customVersion.trimmingCharacters(in: .whitespacesAndNewlines)
                            if !version.isEmpty {
                                store.nativeSetSetting(["claude_version"], .string(version))
                                editingVersion = false
                            }
                        }.disabled(customVersion.isEmpty)
                    }
                }
                NativeSettingsNote(text: "A rebuild takes about 2–5 minutes. New VMs use the new Debian image; running VMs keep theirs. A launch may choose another Claude Code version.")
                Button(golden["rebuilding"].flag ? "Rebuilding…" : "Rebuild image") {
                    Task {
                        await store.saveSettings(store.settings)
                        if store.error == nil {
                            _ = await store.perform("/api/golden/rebuild", success: "VM image rebuild started")
                        }
                        await loadGolden()
                    }
                }.buttonStyle(.borderedProminent)
                    .disabled(golden.isNull || golden["rebuilding"].flag || store.busy.contains("/api/golden/rebuild"))
                if !golden["last_result"].text.isEmpty {
                    Text(golden["last_result"].text == "ok" ? "Last rebuild succeeded." : "Last rebuild: \(golden["last_result"].text)")
                }
                NativeSettingsError(message: store.error)
            }
            if !golden["log_tail"].text.isEmpty {
                Section("Rebuild log") {
                    Text(golden["log_tail"].text).font(.system(.caption, design: .monospaced)).textSelection(.enabled)
                }
            }
        }
        .task {
            customVersion = store.settings["claude_version"].text
            // Local image status must not wait for the remote release catalogue.
            await loadGolden()
            do { releases = try await store.api.request("/api/claude/versions") }
            catch is CancellationError { return }
            catch { if Task.isCancelled { return }; store.error = error.localizedDescription }
            while !Task.isCancelled {
                await loadGolden()
                do { try await Task.sleep(for: .seconds(golden["rebuilding"].flag ? 1.5 : 5)) }
                catch { break }
            }
        }
    }

    private func loadGolden() async {
        do { golden = try await store.api.request("/api/golden") }
        catch is CancellationError {}
        catch { if !Task.isCancelled { store.error = error.localizedDescription } }
    }
}
