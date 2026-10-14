import AppKit
import SwiftUI

struct NativeLauncherView: View {
    @ObservedObject var store: NativeStore
    @State private var repo = ""
    @State private var check: JSON = .null
    @State private var branch = ""
    @State private var model = "default"
    @State private var version = ""
    @State private var cpus = 4
    @State private var memory = 4096
    @State private var interactive = true
    @State private var prompt = ""
    @State private var launchError: String?
    @State private var launching = false
    @State private var initialized = false
    @State private var releases: [String] = []
    @State private var customModel = false
    @State private var customVersion = false

    private var recent: [String] {
        NativeLauncherLogic.recent([store.settings["default_repo"].text]
            + (UserDefaults.standard.stringArray(forKey: "native.recentRepos") ?? [])
            + store.tasks.map { $0["repo"].text })
    }
    private var blocker: String? {
        NativeLauncherLogic.blocker(status: store.status, connected: store.connected, repo: repo, check: check,
                                   interactive: interactive, prompt: prompt)
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
                Section("Claude and resources") {
                    Picker("Model", selection: Binding(get: { customModel ? "__custom" : model }, set: {
                        customModel = $0 == "__custom"
                        if !customModel { model = $0 }
                    })) {
                        Text("Default (Opus)").tag("default")
                        Text("Opus").tag("opus")
                        Text("Sonnet").tag("sonnet")
                        Text("Haiku").tag("haiku")
                        Text("Custom model ID…").tag("__custom")
                    }
                    if customModel { TextField("Model ID", text: $model) }
                    Picker("Claude Code version", selection: Binding(get: { customVersion ? "__custom" : version }, set: {
                        customVersion = $0 == "__custom"
                        if !customVersion { version = $0 }
                    })) {
                        Text("Image version").tag("")
                        Text("Latest").tag("latest")
                        Text("Stable").tag("stable")
                        ForEach(releases, id: \.self) { Text($0).tag($0) }
                        Text("Exact version…").tag("__custom")
                    }
                    if customVersion { TextField("Exact version", text: $version, prompt: Text("2.1.294")) }
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
                        Text("Task for Claude").font(.headline)
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
                    .buttonStyle(.borderedProminent).disabled(blocker != nil || launching || model.trimmingCharacters(in: .whitespaces).isEmpty)
                    .keyboardShortcut(.return, modifiers: .command)
                    .accessibilityIdentifier("launcher.launch")
            }
        }
        .padding(22).frame(width: 660, height: interactive ? 610 : 740)
        .onChange(of: store.settings["model"].text, initial: true) { _, value in
            guard !initialized, !value.isEmpty else { return }
            initialized = true
            model = value
            customModel = !["default", "opus", "sonnet", "haiku"].contains(value)
            cpus = max(1, store.settings["cpus"].integer)
            memory = max(1024, store.settings["memory_mb"].integer)
            repo = recent.first ?? ""
        }
        .onChange(of: repo) { _, _ in branch = ""; check = .null }
        .task(id: repo) { await checkRepository() }
        .task {
            do {
                let result = try await store.api.request("/api/claude/versions")
                if !Task.isCancelled { releases = result["versions"].items.map(\.text) }
            } catch is CancellationError { }
            catch { if !Task.isCancelled { launchError = error.localizedDescription } }
        }
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
        let body = JSON.object([
            "repo_path": .string(path), "prompt": .string(interactive ? "" : prompt), "branch": branch.isEmpty ? .null : .string(branch),
            "interactive": .bool(interactive), "model": .string(model.trimmingCharacters(in: .whitespaces)),
            "claude_version": version.isEmpty ? .null : .string(version), "cpus": .number(Double(cpus)), "memory_mb": .number(Double(memory))
        ])
        guard let created = await store.perform("/api/tasks", body: body), !created["id"].text.isEmpty else {
            launchError = store.error
            return
        }
        UserDefaults.standard.set(NativeLauncherLogic.recent([path] + recent), forKey: "native.recentRepos")
        store.launcherPresented = false
        store.route = .machine(created["id"].text)
    }
}
