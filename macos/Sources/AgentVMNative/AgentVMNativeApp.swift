import AppKit
import SwiftUI

@main
struct AgentVMNativeApp: App {
    @StateObject private var backend = BackendController()
    @AppStorage("native.appearance") private var appearance = "system"
    @Environment(\.openWindow) private var openWindow

    private var scheme: ColorScheme? { appearance == "system" ? nil : appearance == "light" ? .light : .dark }
    private var title: String {
        #if NATIVE_UI_QA
        "agentvm Verification :\(ProcessInfo.processInfo.environment["AGENTVM_PORT"] ?? "isolated")"
        #else
        "agentvm Native"
        #endif
    }

    init() {
        if CommandLine.arguments.contains("--check-resources") {
            do {
                try NativeResourceBundle.verify(in: .main)
                print("Native app resources verified.")
                exit(0)
            } catch {
                fputs(error.localizedDescription + "\n", stderr)
                exit(1)
            }
        }
    }

    var body: some Scene {
        WindowGroup(title, id: "workspace") {
            Group {
                if let store = backend.store {
                    NativeWorkspace(store: store)
                } else {
                    VStack(spacing: 18) {
                        Image(systemName: "square.stack.3d.up.fill").font(.system(size: 48)).foregroundStyle(.indigo)
                        Text("agentvm").font(.largeTitle.bold())
                        if let error = backend.error {
                            Text(error).foregroundStyle(.secondary).multilineTextAlignment(.center)
                            HStack {
                                Button("Try again") { Task { await backend.connect() } }
                                Button("Open logs") { backend.showLogs() }
                            }
                        } else {
                            ProgressView("Connecting to your machines…")
                        }
                    }.padding(48).frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .frame(minWidth: 740, minHeight: 520)
            .preferredColorScheme(scheme)
            .task { if backend.store == nil { await backend.connect() } }
        }
        .defaultSize(width: 1400, height: 900)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("New VM") { backend.store?.launcherPresented = true }
                    .keyboardShortcut("k", modifiers: .command)
                    .disabled(backend.store == nil)
            }
            CommandGroup(replacing: .appSettings) {
                Button("Settings…") { backend.store?.route = .settings; showWindow() }
                    .keyboardShortcut(",", modifiers: .command)
            }
            CommandMenu("Machines") {
                Button("Show Machines") { backend.store?.route = .machines }
                    .keyboardShortcut("1", modifiers: .command)
                Button("Show Snapshots") { backend.store?.route = .snapshots }
                    .keyboardShortcut("2", modifiers: .command)
                Button("Refresh") { Task { await backend.store?.refresh() } }
                    .keyboardShortcut("r", modifiers: .command)
                Button("Show or Hide Machine Details") {
                    NotificationCenter.default.post(name: Notification.Name("AgentVMNativeToggleInspector"), object: nil)
                }.keyboardShortcut("j", modifiers: .command)
                Divider()
                Button("Open Dashboard in Browser") {
                    if let url = backend.store?.api.baseURL { NSWorkspace.shared.open(url) }
                }
                Button("Open Data Folder") {
                    if let config = try? RuntimeConfiguration(environment: ProcessInfo.processInfo.environment) {
                        NSWorkspace.shared.open(config.home)
                    }
                }
            }
            #if NATIVE_UI_QA
            CommandMenu("Verification") {
                Button("Compact Window") {
                    if let window = NSApp.keyWindow ?? NSApp.mainWindow {
                        window.setContentSize(NSSize(width: 780, height: 900))
                        window.center()
                    }
                }.keyboardShortcut("7", modifiers: [.command, .option])
                Button("Wide Window") {
                    if let window = NSApp.keyWindow ?? NSApp.mainWindow {
                        window.setContentSize(NSSize(width: 1400, height: 900))
                        window.center()
                    }
                }.keyboardShortcut("8", modifiers: [.command, .option])
            }
            #endif
        }
        MenuBarExtra("agentvm Native", systemImage: "square.stack.3d.up") {
            Text("\(backend.store?.status["running"].integer ?? 0) machines running")
            Button("Open agentvm") { showWindow() }
            Button("New VM") { backend.store?.launcherPresented = true; showWindow() }
            Divider()
            Button("Quit agentvm Native") { NSApp.terminate(nil) }
        }
    }

    private func showWindow() {
        let windows = NSApp.windows.filter { $0.styleMask.contains(.titled) }
        if let index = NativeWindowPolicy.existingIndex(title: title, titles: windows.map(\.title)) {
            let window = windows[index]
            if window.isMiniaturized { window.deminiaturize(nil) }
            window.makeKeyAndOrderFront(nil)
        } else {
            openWindow(id: "workspace")
        }
        NSApp.activate(ignoringOtherApps: true)
    }
}
