import SwiftUI

struct NativeGitPane: View {
    @ObservedObject var store: NativeStore
    @State private var hosts: [String] = []
    @State private var host = "github.com"
    @State private var deleting: String?
    @State private var confirmDelete = false

    var body: some View {
        Form {
            Section("Saved tokens") {
                if hosts.isEmpty { NativeSettingsNote(text: "No git tokens saved.") }
                ForEach(hosts, id: \.self) { name in
                    HStack {
                        Label(name, systemImage: "key")
                        Spacer()
                        Text("Token saved").foregroundStyle(.green)
                        Button("Remove", role: .destructive) { deleting = name; confirmDelete = true }
                    }
                }
            }
            Section("Add or replace a token") {
                TextField("Host", text: $host)
                HStack {
                    ForEach(["github.com", "gitlab.com", "bitbucket.org"], id: \.self) { name in
                        Button(name) { host = name }.buttonStyle(.borderless)
                    }
                }
                NativeSecretField(label: "Git token", placeholder: "github_pat_…", saved: hosts.contains(host)) { secret in
                    let name = host.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
                    guard !name.isEmpty else { store.error = "Enter a git host."; return false }
                    let path = name.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? name
                    let result = await store.perform("/api/settings/git/\(path)", method: "PUT",
                                                     body: .object(["token": .string(secret)]),
                                                     success: "Git token saved in the Keychain")
                    await load()
                    return result != nil
                }
                NativeSettingsNote(text: "Private repositories use this Mac's ssh key, gh auth login, or a token here. Git tokens stay on this Mac.")
                if host == "github.com" {
                    Link("Create a GitHub token", destination: URL(string: "https://github.com/settings/personal-access-tokens/new")!)
                    NativeSettingsNote(text: "Grant Contents read and write for the repositories you need.")
                } else if host == "gitlab.com" {
                    Link("Create a GitLab token", destination: URL(string: "https://gitlab.com/-/user_settings/personal_access_tokens")!)
                    NativeSettingsNote(text: "Grant read_repository and write_repository.")
                }
                NativeSettingsError(message: store.error)
            }
        }
        .task { await load() }
        .confirmationDialog("Remove the token for \(deleting ?? "this host")?", isPresented: $confirmDelete) {
            Button("Remove token", role: .destructive) {
                guard let deleting else { return }
                Task {
                    let path = deleting.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? deleting
                    _ = await store.perform("/api/settings/git/\(path)", method: "DELETE")
                    await load()
                }
            }
        }
    }

    private func load() async {
        do { hosts = try await store.api.request("/api/settings/git")["hosts"].items.map(\.text) }
        catch is CancellationError {}
        catch { if !Task.isCancelled { store.error = error.localizedDescription } }
    }
}

struct NativeTailscalePane: View {
    @ObservedObject var store: NativeStore
    @State private var saved = false
    @State private var tagDraft = ""
    @FocusState private var editingTags: Bool

    var body: some View {
        Form {
            Section("Auth key") {
                Link("Generate a Tailscale auth key", destination: URL(string: "https://login.tailscale.com/admin/settings/keys")!)
                NativeSettingsNote(text: "Use a reusable key, and pre-approve it if your tailnet requires approval.")
                NativeSecretField(label: "Tailscale auth key", placeholder: "tskey-auth-…", saved: saved,
                                  save: { secret in
                    let result = await store.perform("/api/settings/tailscale", method: "PUT",
                                                     body: .object(["key": .string(secret)]), success: "Tailscale key saved")
                    await load()
                    return result != nil
                }, remove: {
                    let result = await store.perform("/api/settings/tailscale", method: "DELETE", success: "Tailscale key removed")
                    await load()
                    return result != nil
                })
            }
            Section("New VMs") {
                Toggle("Join the tailnet automatically", isOn: store.nativeFlag(["tailscale", "enabled"]))
                    .disabled(!saved && !store.settings["tailscale"]["enabled"].flag)
                Toggle("Tailscale SSH", isOn: store.nativeFlag(["tailscale", "ssh"]))
                HStack {
                    TextField("ACL tags (comma separated)", text: $tagDraft).focused($editingTags).onSubmit(saveTags)
                    Button("Save tags", action: saveTags)
                }
                NativeSettingsNote(text: "Tags are required for an OAuth client secret. Each VM joins as an ephemeral agent and leaves when it ends.")
                NativeSettingsError(message: store.error)
            }
        }.task {
            tagDraft = store.settings["tailscale"]["tags"].items.map(\.text).joined(separator: ", ")
            await load()
        }
        .onChange(of: store.settings["tailscale"]["tags"]) { _, tags in
            if !editingTags { tagDraft = tags.items.map(\.text).joined(separator: ", ") }
        }
    }

    private func saveTags() {
        let tags = tagDraft.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        store.nativeSetSetting(["tailscale", "tags"], .array(tags.map(JSON.string)))
    }

    private func load() async {
        do { saved = try await store.api.request("/api/settings/tailscale")["key_saved"].flag }
        catch is CancellationError {}
        catch { if !Task.isCancelled { store.error = error.localizedDescription } }
    }
}
