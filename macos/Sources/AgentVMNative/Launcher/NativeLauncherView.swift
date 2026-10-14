import AppKit
import SwiftUI

struct NativeLauncherView: View {
    @ObservedObject var store: NativeStore
    @State private var repo = ""
    @State private var check: JSON = .null
    @State private var branch = ""
    @State private var version = ""
    @State private var cpus = 4
    @State private var memory = 4096
    @State private var interactive = true
    @State private var prompt = ""
    @State private var launchError: String?
    @State private var launching = false
    @State private var initialized = false
    @State private var versionCatalog = NativeClaudeVersionCatalog()
    @State private var customVersion = false
    @State private var selection: JSON = .object(["kind": .string("claude")])
    @State private var custom: JSON = .null
    @State private var rememberedProviders: [String: String] = [:]

    private var supported: Bool { store.agentCatalogState == .supported }
    private var catalog: JSON { supported ? store.agentCatalog : .null }
    private var selectedKind: String { supported ? NativeAgentPolicy.kind(selection) : "claude" }
    private var versionSelectionReady: Bool { initialized && (supported || store.agentCatalogState == .legacy) }
    private var launchSelection: JSON {
        var current = selection
        if NativeAgentPolicy.kind(current) == "custom" { current["custom"] = custom }
        current["version"] = version.isEmpty ? .null : .string(version)
        return current
    }
    private var agentName: String { NativeAgentPolicy.displayName(supported ? launchSelection : .null, catalog: catalog) }

    private var recent: [String] {
        NativeLauncherLogic.recent([store.settings["default_repo"].text]
            + (UserDefaults.standard.stringArray(forKey: "native.recentRepos") ?? [])
            + store.tasks.map { $0["repo"].text })
    }
    private var blocker: String? {
        if !store.connected { return "Connecting to agentvm…" }
        if !store.status["golden"].flag { return "Build the VM image in Settings → VM image." }
        if case .loading = store.agentCatalogState { return "Loading coding CLIs…" }
        if case .failed(let error) = store.agentCatalogState { return error }
        if let error = NativeAgentPolicy.readiness(selection: launchSelection, catalog: catalog, legacyToken: store.status["token"].flag,
                                                  interactive: interactive) { return error }
        return NativeLauncherLogic.blocker(status: store.status, connected: store.connected, repo: repo, check: check,
                                          interactive: interactive, prompt: prompt, requireClaudeToken: false, agentName: agentName)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack {
                Text("New VM").font(.title.bold())
                Spacer()
                Button("Cancel") { store.launcherPresented = false }.keyboardShortcut(.cancelAction)
                    .accessibilityIdentifier("launcher.cancel")
            }
            Form {
                Section("Repository") {
                    HStack {
                        TextField("Folder or repository link", text: $repo, prompt: Text("~/code/app or github.com/owner/repo"))
                            .accessibilityIdentifier("launcher.repository")
                        Button("Choose folder…", action: chooseFolder)
                    }
                    if !recent.isEmpty {
                        Menu("Recent repositories") {
                            ForEach(recent, id: \.self) { path in Button(path) { repo = path } }
                        }
                    }
                    repositoryStatus
                    if check["ok"].flag && check["branches"].items.count > 1 {
                        Picker("Branch", selection: $branch) {
                            Text(check["default_branch"].text.isEmpty ? "Current HEAD" : "\(check["default_branch"].text) (default)").tag("")
                            ForEach(check["branches"].items.map(\.text).filter { $0 != check["default_branch"].text }, id: \.self) {
                                Text($0).tag($0)
                            }
                        }
                    }
                }
                Section("Agent and resources") {
                    NativeAgentFields(catalog: catalog, supported: supported, releases: versionCatalog.releases, selection: $selection,
                                      version: $version, custom: $custom, customVersion: $customVersion,
                                      rememberedProviders: $rememberedProviders, configureProviders: configureProviders,
                                      configureAccounts: configureAccounts)
                    if versionSelectionReady, let error = versionCatalog.visibleError(kind: selectedKind) {
                        Text("Could not load Claude versions: \(error)").font(.caption).foregroundStyle(.red)
                    }
                    Stepper("vCPUs: \(cpus)", value: $cpus, in: 1...max(1, store.limits["cpus"].integer))
                        .accessibilityIdentifier("launcher.cpus")
                    Picker("Memory", selection: $memory) {
                        ForEach(NativeLauncherLogic.memoryChoices(host: store.limits["ram_mb"].integer, current: memory), id: \.self) {
                            Text(MachineFormat.memory(Double($0))).tag($0)
                        }
                    }.accessibilityIdentifier("launcher.memory")
                    resourceHint
                    Toggle("Interactive terminal", isOn: $interactive).accessibilityIdentifier("launcher.interactive")
                    if !interactive {
                        Text("Task for \(agentName)").font(.headline)
                        TextEditor(text: $prompt).frame(minHeight: 90).border(.quaternary)
                        Text("Automatic tasks run without a terminal and stop at the time limit in Settings.")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
            }.formStyle(.grouped)
            if let launchError { Text(launchError).foregroundStyle(.red).textSelection(.enabled) }
            HStack {
                if let blocker { Text(blocker).font(.callout).foregroundStyle(.secondary) }
                Spacer()
                Button(launching ? "Launching…" : "Launch VM") { Task { await launch() } }
                    .buttonStyle(.borderedProminent).disabled(blocker != nil || launching)
                    .keyboardShortcut(.return, modifiers: .command)
                    .accessibilityIdentifier("launcher.launch")
                if case .failed = store.agentCatalogState {
                    Button("Retry") { Task { await store.refreshAgents() } }.accessibilityIdentifier("launcher.catalog.retry")
                }
            }
        }
        .padding(22).frame(width: 660, height: interactive ? 610 : 740)
        .onChange(of: store.settings, initial: true) { _, _ in initialize() }
        .onChange(of: store.agentCatalogState, initial: true) { _, state in
            if state == .legacy && initialized && NativeAgentPolicy.kind(selection) != "claude" {
                selection = .object(["kind": .string("claude")])
                version = ""
                customVersion = false
            }
            initialize()
        }
        .onChange(of: repo) { _, _ in branch = ""; check = .null }
        .task(id: repo) { await checkRepository() }
        .task { await store.refreshAgents() }
        .task(id: "\(versionSelectionReady)/\(selectedKind)") { await loadClaudeVersions() }
    }

    private func loadClaudeVersions() async {
        guard versionCatalog.shouldLoad(kind: selectedKind, initialized: versionSelectionReady) else { return }
        do {
            let result = try await store.api.request("/api/claude/versions")
            if !Task.isCancelled { versionCatalog.receive(result["versions"].items.map(\.text)) }
        } catch is CancellationError { }
        catch { if !Task.isCancelled { versionCatalog.fail(error.localizedDescription) } }
    }

    private func initialize() {
        guard !initialized, !store.settings.isNull, store.agentCatalogState == .supported || store.agentCatalogState == .legacy else { return }
        initialized = true
        selection = supported && !store.settings["agent"].isNull ? store.settings["agent"] : .object(["kind": .string("claude")])
        custom = NativeAgentPolicy.initialCustom(settings: store.settings)
        version = supported ? selection["version"].text : ""
        customVersion = !["", "latest", "stable"].contains(version)
        cpus = max(1, store.settings["cpus"].integer)
        memory = max(1024, store.settings["memory_mb"].integer)
        repo = recent.first ?? ""
    }

    private func configureProviders() {
        UserDefaults.standard.set("providers", forKey: "native.settingsSection")
        store.launcherPresented = false
        store.route = .settings
    }

    private func configureAccounts() {
        UserDefaults.standard.set("accounts", forKey: "native.settingsSection")
        store.launcherPresented = false
        store.route = .settings
    }

    private var repositoryStatus: some View {
        VStack(alignment: .leading, spacing: 6) {
            if repo.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                Text("The VM gets a fresh clone. Your files stay on your Mac.")
            } else if check.isNull {
                HStack { ProgressView().controlSize(.small); Text("Checking repository…") }
            } else if !check["ok"].flag {
                Text(check["error"].text).foregroundStyle(.red)
                if check["visibility"].text == "no_access" {
                    Button("Add a Git token in Settings") {
                        UserDefaults.standard.set("git", forKey: "native.settingsSection")
                        store.launcherPresented = false
                        store.route = .settings
                    }
                }
            } else {
                Label("\(check["name"].text) · \(NativeLauncherLogic.repoReference(check))", systemImage: "checkmark.circle.fill")
                    .foregroundStyle(.green)
                if !check["remote"].isNull {
                    if check["visibility"].text == "private" { Text("Private · this Mac has access") }
                    else if check["visibility"].text == "public" { Text("Public repository") }
                    Text(check["to_clone"].flag ? "Cloned at launch into \(check["path"].text)" : "Fetched at launch")
                }
                if !check["warning"].text.isEmpty { Text(check["warning"].text).foregroundStyle(.orange) }
            }
        }.font(.caption).foregroundStyle(.secondary)
    }

    private var resourceHint: some View {
        VStack(alignment: .leading, spacing: 4) {
            if let capacity = NativeLauncherLogic.capacity(status: store.status, memory: memory) {
                Text(capacity.waitingFor == "concurrency"
                    ? "This VM waits in the queue: \(store.status["concurrency"].integer) run at a time."
                    : capacity.waitingFor == "memory"
                    ? "This VM waits in the queue: no memory free for \(MachineFormat.memory(Double(memory))) right now."
                    : "\(MachineFormat.memory(Double(capacity.freeMB))) free for VMs: room for \(capacity.fit) like this.")
                    .foregroundStyle(capacity.starts == 0 ? Color.orange : Color.secondary)
                    .accessibilityIdentifier("launcher.capacity")
            }
            if cpus >= store.limits["cpus"].integer && store.limits["cpus"].integer > 1 {
                Text("Every VM shares this Mac's cores.").foregroundStyle(.secondary)
            }
        }.font(.caption)
    }

    private func chooseFolder() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let path = panel.url?.path { repo = path }
    }

    private func checkRepository() async {
        let path = repo.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !path.isEmpty else { return }
        do {
            try await Task.sleep(for: .milliseconds(150))
            let result = try await store.api.request("/api/repos/check?path=\(NativeInspectorLogic.percentQuery(path))")
            if !Task.isCancelled { check = result }
        } catch is CancellationError { }
        catch {
            if !Task.isCancelled { check = .object(["ok": .bool(false), "error": .string(error.localizedDescription)]) }
        }
    }

    private func launch() async {
        guard blocker == nil, !launching else { return }
        launching = true
        launchError = nil
        defer { launching = false }
        let path = repo.trimmingCharacters(in: .whitespacesAndNewlines)
        let base = JSON.object([
            "repo_path": .string(path), "prompt": .string(interactive ? "" : prompt), "branch": branch.isEmpty ? .null : .string(branch),
            "interactive": .bool(interactive), "cpus": .number(Double(cpus)), "memory_mb": .number(Double(memory))
        ])
        let body = NativeAgentPolicy.launchBody(base: base, selection: launchSelection, version: version, supported: supported, catalog: catalog)
        guard let created = await store.perform("/api/tasks", body: body), !created["id"].text.isEmpty else {
            launchError = store.error
            return
        }
        UserDefaults.standard.set(NativeLauncherLogic.recent([path] + recent), forKey: "native.recentRepos")
        store.launcherPresented = false
        store.route = .machine(created["id"].text)
    }
}
