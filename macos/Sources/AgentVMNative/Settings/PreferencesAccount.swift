import SwiftUI
import UserNotifications

struct NativePreferencesPane: View {
    @AppStorage("native.appearance") private var appearance = "system"
    @AppStorage("native.notifications") private var notifications = false
    @State private var notificationMessage: String?

    var body: some View {
        Form {
            Section("Appearance") {
                Picker("Theme", selection: $appearance) {
                    Text("System").tag("system")
                    Text("Light").tag("light")
                    Text("Dark").tag("dark")
                }.pickerStyle(.segmented)
                NativeSettingsNote(text: "Saved for this native application. Terminals follow the chosen appearance.")
            }
            Section("Notifications") {
                Toggle("Notify when Claude is waiting", isOn: $notifications)
                    .onChange(of: notifications) { _, enabled in
                        guard enabled else { return }
                        Task {
                            do {
                                let granted = try await UNUserNotificationCenter.current()
                                    .requestAuthorization(options: [.alert, .sound])
                                if !granted {
                                    notifications = false
                                    notificationMessage = "Allow notifications for AgentVM in System Settings."
                                }
                            } catch {
                                notifications = false
                                notificationMessage = error.localizedDescription
                            }
                        }
                    }
                NativeSettingsNote(text: "An alert when an agent finishes a turn while AgentVM is in the background.")
                NativeSettingsError(message: notificationMessage)
            }
            Section("Startup") { NativeLoginToggle() }
        }
    }
}

struct NativeSecretField: View {
    let label: String
    let placeholder: String
    let saved: Bool
    let save: @MainActor (String) async -> Bool
    var remove: (@MainActor () async -> Bool)?
    @State private var value = ""
    @State private var saving = false
    @State private var confirmRemove = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if saved { Label("Saved in the macOS Keychain", systemImage: "checkmark.shield").foregroundStyle(.green) }
            HStack {
                SecureField(saved ? "Paste a replacement" : placeholder, text: $value)
                    .accessibilityLabel(label)
                Button(saving ? "Saving…" : "Save") {
                    let secret = value.trimmingCharacters(in: .whitespacesAndNewlines)
                    value = ""
                    saving = true
                    Task { _ = await save(secret); saving = false }
                }.disabled(saving || value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                if saved, remove != nil {
                    Button("Remove", role: .destructive) { confirmRemove = true }.disabled(saving)
                }
            }
        }
        .confirmationDialog("Remove \(label)?", isPresented: $confirmRemove) {
            Button("Remove", role: .destructive) {
                saving = true
                Task { _ = await remove?(); saving = false }
            }
        }
        .onDisappear { value = "" }
    }
}

struct NativeAccountPane: View {
    @ObservedObject var store: NativeStore
    var body: some View {
        Form {
            Section("Claude subscription") {
                LabeledContent("Status", value: store.status["token"].flag ? "Connected" : "Token missing")
                NativeSettingsNote(text: store.status["token"].flag
                                   ? "New VMs use this account. Running VMs keep their starting token."
                                   : store.status["token_hint"].text)
                NativeSecretField(label: "Claude token", placeholder: "sk-ant-oat01-…",
                                  saved: store.status["token"].flag) { secret in
                    await store.perform("/api/settings/token", method: "PUT",
                                        body: .object(["token": .string(secret)]),
                                        success: "Claude token saved in the Keychain") != nil
                }
                NativeSettingsNote(text: "Create a subscription token with claude setup-token in Terminal.")
                NativeSettingsError(message: store.error)
            }
        }
    }
}
