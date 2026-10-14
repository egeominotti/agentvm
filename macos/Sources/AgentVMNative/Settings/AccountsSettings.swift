import AppKit
import SwiftUI

struct NativeAccountsSettingsPane: View {
    @ObservedObject var store: NativeStore
    private var supported: Bool { NativeAgentAuthentication.supported(catalog: store.agentCatalog) }

    var body: some View {
        Form {
            Section("Account access") {
                NativeAgentCatalogStatus(store: store)
                NativeSettingsNote(text: "Connect supported service accounts on this Mac. Account access, subscription benefits and credit plans depend on the service; provider API keys are configured separately.")
                if !supported && store.agentCatalogState == .supported {
                    NativeSettingsNote(text: "Account connections require a newer AgentVM engine. The existing Claude token setting is still available.")
                }
                Button("Open Claude token settings…") { UserDefaults.standard.set("account", forKey: "native.settingsSection") }
                Button("Refresh account status", systemImage: "arrow.clockwise") { Task { await store.refreshAgents() } }
                    .disabled(store.agentCatalogState == .loading)
            }
            if supported {
                ForEach(store.agentCatalog["accounts"].items, id: \.id) { account in
                    Section(account["name"].text) { NativeAccountConnectionCard(store: store, account: account) }
                }
            }
        }
        .task { await store.refreshAgents() }
    }
}

private struct NativeAccountConnectionCard: View {
    @ObservedObject var store: NativeStore
    let account: JSON
    @ObservedObject private var login: NativeAccountLoginState
    @Environment(\.scenePhase) private var scenePhase
    @State private var working = false
    @State private var confirmRemove = false
    @State private var actionError: String?
    private var path: String { "/api/settings/accounts/\(account.id)" }
    private var busy: Bool { working || store.busy.contains(where: { $0 == path || $0.hasPrefix(path + "/") }) }
    private var ready: Bool { store.agentCatalogState == .supported }
    private var removable: Bool { account["connected"].flag || !account["source"].text.isEmpty || !account["error"].text.isEmpty }
    private var pollIdentity: String { "\(login.session?.id ?? "")-\(scenePhase)-\(login.retryGeneration)" }

    @MainActor init(store: NativeStore, account: JSON) {
        self.store = store
        self.account = account
        login = store.accountLoginState(account.id)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            LabeledContent("Status", value: account["connected"].flag ? "Connected" : "Not connected")
            if !account["source"].text.isEmpty {
                Text("Source: \(account["source"].text)").foregroundStyle(.secondary).textSelection(.enabled)
            }
            NativeSettingsError(message: account["error"].text.isEmpty ? nil : account["error"].text)
            ViewThatFits(in: .horizontal) {
                HStack { actions }
                VStack(alignment: .leading, spacing: 8) { actions }
            }.disabled(!ready || busy)
            NativeSettingsNote(text: "Import references an existing host CLI sign-in. Sign in opens the service's verification page; account credentials remain on this Mac.")
            if let session = login.session { loginStatus(session) }
            NativeSettingsError(message: actionError)
        }
        .confirmationDialog("Remove \(account["name"].text) from AgentVM?", isPresented: $confirmRemove) {
            Button("Remove account connection", role: .destructive) { runAction("", method: "DELETE") }
        } message: {
            Text("This removes the AgentVM connection. Referenced host CLI sign-ins are retained.")
        }
        .task(id: pollIdentity) { await pollLogin() }
    }

    @ViewBuilder private var actions: some View {
        Button("Import host login") { runAction("/import") }
            .disabled(!account["can_import"].isNull && !account["can_import"].flag)
        Button(working ? "Working…" : "Sign in…") { startLogin() }
            .disabled(!account["can_login"].isNull && !account["can_login"].flag)
        if removable { Button("Remove…", role: .destructive) { confirmRemove = true } }
    }

    @ViewBuilder private func loginStatus(_ session: NativeAccountLoginSession) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            if let code = session.userCode {
                Text("Verification code").font(.caption).foregroundStyle(.secondary)
                Text(code).font(.title3.monospaced().bold()).textSelection(.enabled)
            }
            switch login.phase {
            case .pending:
                Text(scenePhase == .active ? "Waiting for browser sign-in…" : "Sign-in checks paused while the app is inactive.")
                    .foregroundStyle(.secondary)
                Button("Open verification page") { _ = NSWorkspace.shared.open(session.verificationURL) }
                    .disabled(Date() >= session.expiresAt)
            case .connected: Label("Account connected", systemImage: "checkmark.shield").foregroundStyle(.green)
            case .failed(let message):
                NativeSettingsError(message: message)
                if Date() < session.expiresAt {
                    Button("Check sign-in again") { login.retry() }
                }
            }
        }
    }

    private func runAction(_ suffix: String, method: String = "POST") {
        guard ready, !busy else { return }
        working = true
        actionError = nil
        Task {
            let result = await store.perform(path + suffix, method: method,
                                             success: method == "DELETE" ? "Account connection removed" : "Host account connection imported")
            if result != nil { login.clear(); await store.refreshAgents() }
            else { actionError = store.error }
            working = false
        }
    }

    private func startLogin() {
        guard ready, !busy else { return }
        working = true
        actionError = nil
        login.clear()
        Task {
            defer { working = false }
            guard let response = await store.perform(path + "/login") else { actionError = store.error; return }
            do {
                let next = try NativeAccountLoginPolicy.session(response: response, now: Date())
                login.begin(next)
                if !NSWorkspace.shared.open(next.verificationURL) {
                    actionError = "The verification page could not be opened. Use Open verification page to try again."
                }
            } catch { actionError = error.localizedDescription }
        }
    }

    private func pollLogin() async {
        guard let session = login.session else { return }
        while !Task.isCancelled && login.isCurrent(session.id) {
            switch NativeAccountLoginPolicy.pollDecision(session: session, phase: login.phase, polls: login.polls,
                                                         now: Date(), active: scenePhase == .active) {
            case .paused, .finished: return
            case .expired: login.setPhase(.failed("Sign-in expired. Start sign-in again."), for: session.id); return
            case .limitReached: login.setPhase(.failed("Sign-in checks paused after repeated attempts. Check again or start a new sign-in."), for: session.id); return
            case .request: break
            }
            do {
                login.polls += 1
                let response = try await store.api.request(path + "/login/" + session.id)
                try Task.checkCancellation()
                guard login.acceptResponse(response, for: session.id) else { return }
                if login.phase == .connected { await store.refreshAgents(); return }
                if login.phase != .pending { return }
                try await Task.sleep(for: .seconds(2))
            } catch is CancellationError { return }
            catch { if !Task.isCancelled { login.setPhase(.failed(error.localizedDescription), for: session.id) }; return }
        }
    }
}
