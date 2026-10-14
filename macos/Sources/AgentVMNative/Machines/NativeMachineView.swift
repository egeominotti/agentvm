import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct NativeMachineView: View {
    @ObservedObject var store: NativeStore
    let id: String
    @State private var session = "claude"
    @State private var extras: [String]
    @State private var mounted: Set<String>
    @State private var details = true
    @State private var inspectorTab = "telemetry"
    @State private var showFiles = false
    @State private var dropping = false
    @State private var destructive: String?
    @State private var confirmDestructive = false

    init(store: NativeStore, id: String) {
        self.store = store
        self.id = id
        let restored = NativeMachineLogic.restoreSessions(stored: UserDefaults.standard.string(forKey: "native.activeSession.\(id)"),
            extras: UserDefaults.standard.stringArray(forKey: "native.shells.\(id)") ?? [])
        _session = State(initialValue: restored.active)
        _extras = State(initialValue: restored.extras)
        _mounted = State(initialValue: restored.mounted)
    }

    private var task: JSON { store.tasks.first { $0["id"].text == id } ?? .null }
    private var base: String { "/api/tasks/\(id)" }
    private var running: Bool { NativeMachineLogic.state(task) == "running" }

    var body: some View {
        Group {
            if task.isNull {
                ContentUnavailableView("Machine not found", systemImage: "desktopcomputer", description: Text("It may have been removed from the list."))
            } else {
                VStack(spacing: 0) {
                    NativeMachineToolbar(store: store, task: task, details: $details, showFiles: $showFiles,
                                         destructive: { action in destructive = action; confirmDestructive = true })
                    Divider()
                    if task["interactive"].flag && !NativeMachineLogic.ended(task) { sessionTabs }
                    if !NativeMachineLogic.ended(task) { NativeMachineConnectivity(store: store, task: task) }
                    if !NativeMachineLogic.ended(task) && NativeBootLogic.setupFailed(task) {
                        HStack {
                            Label("Repository setup failed. This VM runs without what .agentvm/setup.sh installs.", systemImage: "exclamationmark.triangle")
                                .font(.caption).foregroundStyle(.orange)
                            Spacer()
                            Button("Show setup log") {
                                details = true
                                inspectorTab = "diagnostics"
                            }
                        }.padding(10)
                    }
                    HSplitView {
                        stage.frame(minWidth: 280, maxWidth: .infinity, maxHeight: .infinity)
                        if details { NativeInspectorView(store: store, task: task, tab: $inspectorTab).frame(minWidth: 240, idealWidth: 380, maxWidth: 520) }
                    }
                }
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: Notification.Name("AgentVMNativeToggleInspector"))) { _ in details.toggle() }
        .onReceive(NotificationCenter.default.publisher(for: Notification.Name("AgentVMNativeShowConversation"))) { notification in
            if notification.object as? String == id { details = true; inspectorTab = "claude" }
        }
        .onReceive(NotificationCenter.default.publisher(for: Notification.Name("AgentVMNativeShowDiagnostics"))) { notification in
            if notification.object as? String == id { details = true; inspectorTab = "diagnostics" }
        }
        .onChange(of: NativeMachineLogic.state(task), initial: true) { _, state in
            if state == "failed" { inspectorTab = "diagnostics" }
            else if !task.isNull && (NativeMachineLogic.ended(task) || !task["interactive"].flag) { inspectorTab = "claude" }
        }
        .onChange(of: session, initial: true) { _, value in
            UserDefaults.standard.set(value, forKey: "native.activeSession.\(id)")
            NotificationCenter.default.post(name: Notification.Name("AgentVMNativeSessionSelected"), object: nil,
                userInfo: ["id": id, "session": value])
        }
        .confirmationDialog(destructive == "stop" ? "Force stop this VM? Its unsaved disk is kept in Snapshots." : "Delete its logs and agent history? Its branch stays.", isPresented: $confirmDestructive) {
            Button(destructive == "stop" ? "Force stop" : "Delete permanently", role: .destructive) {
                Task {
                    if destructive == "stop" { _ = await store.perform("\(base)/stop") }
                    else if await store.perform(base, method: "DELETE") != nil { store.route = .machines }
                }
            }
        }
        .fileImporter(isPresented: $showFiles, allowedContentTypes: [.item], allowsMultipleSelection: true) { result in
            switch result {
            case .success(let urls): Task { await upload(urls) }
            case .failure(let error): store.error = error.localizedDescription
            }
        }
    }

    private var sessionTabs: some View {
        ScrollView(.horizontal, showsIndicators: false) {
          HStack(spacing: 8) {
            ForEach(["claude", "shell"] + extras, id: \.self) { name in
                HStack(spacing: 4) {
                    Button(NativeAgentPresentation.terminalLabel(name, task: task)) {
                        mounted.insert(name)
                        session = name
                    }.buttonStyle(.bordered).tint(session == name ? .accentColor : .secondary)
                        .accessibilityIdentifier("machine.session.\(name)")
                    if extras.contains(name) {
                        Button { Task { await closeShell(name) } } label: { Image(systemName: "xmark") }
                            .buttonStyle(.plain).help("Close \(NativeMachineLogic.shellLabel(name))")
                            .accessibilityLabel("Close \(NativeMachineLogic.shellLabel(name))")
                            .accessibilityIdentifier("machine.close.\(name)")
                            .disabled(!running || store.busy.contains("\(base)/pty/\(name)"))
                    }
                }
            }
            Button {
                guard let name = NativeMachineLogic.nextShell(extras) else { return }
                extras = NativeMachineLogic.shells(extras + [name])
                mounted.insert(name)
                session = name
                rememberShells()
            } label: { Image(systemName: "plus") }
                .disabled(NativeMachineLogic.nextShell(extras) == nil || !running).help("Open another shell")
                .accessibilityLabel("Open another shell").accessibilityIdentifier("machine.newShell")
            Spacer()
          }.padding(.horizontal, 14).padding(.vertical, 8)
        }.frame(height: 44)
    }

    private var stage: some View {
        Group {
            if NativeMachineLogic.ended(task) {
                NativeMachineOutcome(store: store, task: task)
            } else if !task["interactive"].flag {
                VStack(spacing: 12) {
                    Image(systemName: "sparkles").font(.largeTitle)
                    Text("Automatic task").font(.title2.bold())
                    Text("Follow \(NativeAgentPresentation.name(task)) in the details panel.").foregroundStyle(.secondary)
                    Text(task["prompt"].text).textSelection(.enabled).padding()
                    Button(NativeAgentPresentation.showsClaudeAnalytics(task) ? "Show conversation" : "Show agent output") {
                        details = true
                        inspectorTab = "claude"
                    }
                }.frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ZStack {
                    ForEach(mounted.sorted(), id: \.self) { name in
                        NativeTerminalView(store: store, id: id, session: name, live: running)
                            .opacity(session == name ? 1 : 0)
                            .allowsHitTesting(session == name)
                            .accessibilityHidden(session != name)
                    }
                    if !running || !task["ready"].flag { NativeBootView(store: store, task: task) }
                    if dropping {
                        VStack { Image(systemName: "tray.and.arrow.down").font(.largeTitle); Text("Copy files into the VM") }
                            .padding(28).background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14))
                    }
                }
                .dropDestination(for: URL.self) { urls, _ in
                    guard running else { return false }
                    Task { await upload(urls) }
                    return true
                } isTargeted: { dropping = $0 && running }
            }
        }
    }

    private func closeShell(_ name: String) async {
        guard !store.busy.contains("\(base)/pty/\(name)") else { return }
        let succeeded = await store.perform("\(base)/pty/\(name)", method: "DELETE") != nil
        let updated = NativeMachineLogic.closedSessions(name: name,
            state: NativeRestoredSessions(active: session, extras: extras, mounted: mounted), succeeded: succeeded)
        extras = updated.extras
        mounted = updated.mounted
        session = updated.active
        rememberShells()
    }

    private func rememberShells() { UserDefaults.standard.set(extras, forKey: "native.shells.\(id)") }

    private func upload(_ urls: [URL]) async {
        guard running && task["interactive"].flag else { return }
        let destination = session
        var paths: [String] = []
        for url in urls {
            let access = url.startAccessingSecurityScopedResource()
            defer { if access { url.stopAccessingSecurityScopedResource() } }
            do {
                let result = try await store.api.upload("\(base)/upload", file: url,
                    headers: ["x-file-name": NativeInspectorLogic.percentQuery(url.lastPathComponent)])
                if !result["path"].text.isEmpty { paths.append(result["path"].text) }
            } catch is CancellationError { return }
            catch { store.error = "Could not copy \(url.lastPathComponent): \(error.localizedDescription)" }
        }
        guard !paths.isEmpty else { return }
        let typed = paths.map(NativeMachineLogic.shellQuote).joined(separator: " ") + " "
        NotificationCenter.default.post(name: Notification.Name("AgentVMNativePaste"), object: nil,
            userInfo: ["id": id, "session": destination, "text": typed])
        store.notice = "Copied \(paths.count) files into the VM"
    }
}
