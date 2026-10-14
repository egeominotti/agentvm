import AppKit
import ServiceManagement
import SwiftUI
import UserNotifications

struct NativeLoginToggle: View {
    @State private var enabled = SMAppService.mainApp.status == .enabled
    @State private var error: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Toggle("Open agentvm Native at login", isOn: Binding(get: { enabled }, set: { value in
                do {
                    if value { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
                    enabled = SMAppService.mainApp.status == .enabled
                } catch { self.error = error.localizedDescription }
            }))
            if let error { Text(error).font(.caption).foregroundStyle(.red) }
            if SMAppService.mainApp.status == .requiresApproval {
                Button("Open Login Items") { SMAppService.openSystemSettingsLoginItems() }
            }
        }
    }
}

@MainActor
final class NativeAttention {
    private var waiting: Set<String>?

    func update(_ tasks: [JSON]) {
        let current = Set(tasks.filter { $0.isWaiting && !$0.isEnded }.map(\.id))
        NSApp.dockTile.badgeLabel = current.isEmpty ? nil : String(current.count)
        defer { waiting = current }
        guard let previous = waiting, UserDefaults.standard.bool(forKey: "native.notifications") else { return }
        for task in tasks where current.contains(task.id) && !previous.contains(task.id) {
            let content = UNMutableNotificationContent()
            content.title = "\(NativeAgentPresentation.name(task)) is waiting"
            content.body = task.title
            content.sound = .default
            UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: task.id, content: content, trigger: nil))
        }
    }
}
