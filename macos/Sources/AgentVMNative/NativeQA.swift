import AppKit
import Foundation
import ScreenCaptureKit
import WebKit

// An opt-in end-to-end check captures this app's own views against a real isolated engine.
@MainActor
enum NativeQA {
    private static var started = false

    static func runIfRequested(store: NativeStore) async {
        guard !started, let path = ProcessInfo.processInfo.environment["AGENTVM_NATIVE_QA_DIRECTORY"] else { return }
        started = true
        guard store.api.baseURL.port != 7777 else { return }
        guard let home = try? RuntimeConfiguration(environment: ProcessInfo.processInfo.environment).home,
              home.path.hasPrefix("/tmp/agentvm-native") else { return }
        let folder = URL(fileURLWithPath: path, isDirectory: true)
        let layoutRoutes = Set((ProcessInfo.processInfo.environment["AGENTVM_NATIVE_QA_ROUTES"]
                               ?? "machines,snapshots,settings").split(separator: ",").map(String.init))
        do {
            guard !layoutRoutes.isEmpty, layoutRoutes.isSubset(of: ["machines", "snapshots", "settings"]) else {
                throw RuntimeError.message("QA routes must name machines, snapshots or settings.")
            }
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            try appWindow().makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            await store.refreshSnapshots()
            if ProcessInfo.processInfo.environment["AGENTVM_NATIVE_QA_CONTROLS_ONLY"] != "1" {
                for theme in ["light", "dark"] {
                    UserDefaults.standard.set(theme, forKey: "native.appearance")
                    for width in [1400, 780] {
                        let window = try appWindow()
                        window.setContentSize(NSSize(width: width, height: 900))
                        window.center()
                        for (route, name) in [(NativeRoute.machines, "machines"), (.snapshots, "snapshots"), (.settings, "settings")] {
                            guard layoutRoutes.contains(name) else { continue }
                            store.route = route
                            try await Task.sleep(for: .milliseconds(700))
                            try await capture(folder.appendingPathComponent("\(theme)-\(width)-\(name).png"))
                        }
                        if layoutRoutes.contains("settings") {
                            store.route = .settings
                            for section in NativeSettingsPolicy.catalog {
                                UserDefaults.standard.set(section.id, forKey: "native.settingsSection")
                                try await Task.sleep(for: .milliseconds(300))
                                try await capture(folder.appendingPathComponent("\(theme)-\(width)-settings-\(section.id).png"))
                            }
                        }
                    }
                }
            }
            if ProcessInfo.processInfo.environment["AGENTVM_NATIVE_QA_LAYOUTS_ONLY"] == "1" {
                try Data("Native layout capture completed; real VM terminal was not requested.\n".utf8)
                    .write(to: folder.appendingPathComponent("result.txt"))
                return
            }
            try await terminalCheck(store, folder: folder)
            try Data("Native layouts, settings and real VM terminal passed.\n".utf8)
                .write(to: folder.appendingPathComponent("result.txt"))
        } catch {
            try? Data(error.localizedDescription.utf8).write(to: folder.appendingPathComponent("failure.txt"))
        }
    }

    private static func terminalCheck(_ store: NativeStore, folder: URL) async throws {
        let home = try RuntimeConfiguration(environment: ProcessInfo.processInfo.environment).home
        guard home.path.hasPrefix("/tmp/agentvm-native") else { throw RuntimeError.message("QA requires an isolated temporary home.") }
        let repo = home.appendingPathComponent("qa-repo-\(UUID().uuidString.prefix(8))")
        try FileManager.default.createDirectory(at: repo, withIntermediateDirectories: true)
        try Data("# Native terminal QA\n".utf8).write(to: repo.appendingPathComponent("README.md"))
        for arguments in [["init", "-q", "-b", "main"], ["add", "."],
                          ["-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-qm", "init"]] {
            let command = Process()
            command.executableURL = URL(fileURLWithPath: "/usr/bin/git")
            command.arguments = ["-C", repo.path] + arguments
            command.standardOutput = FileHandle.nullDevice
            command.standardError = FileHandle.nullDevice
            try command.run(); command.waitUntilExit()
            guard command.terminationStatus == 0 else { throw RuntimeError.message("QA repository could not be created.") }
        }
        let created = try await store.api.request("/api/tasks", method: "POST", body: .object([
            "repo_path": .string(repo.path), "interactive": .bool(true), "cpus": JSON(2), "memory_mb": JSON(2048)
        ]))
        let id = created["id"].text
        guard !id.isEmpty else { throw RuntimeError.message("QA VM was not created.") }
        let owned = try await store.api.request("/api/tasks/\(id)")
        guard URL(fileURLWithPath: owned["repo"].text).resolvingSymlinksInPath().path == repo.resolvingSymlinksInPath().path else {
            throw RuntimeError.message("Verification preserves machines created outside its own repository.")
        }
        UserDefaults.standard.set("shell", forKey: "native.activeSession.\(id)")
        await store.refresh()
        store.route = .machine(id)
        do {
            for _ in 0..<300 {
                let task = try await store.api.request("/api/tasks/\(id)")
                if task["ready"].flag { break }
                if task.isEnded { throw RuntimeError.message("QA VM ended before its terminal was ready.") }
                try await Task.sleep(for: .milliseconds(200))
            }
            await store.refresh()
            for theme in ["light", "dark"] {
                UserDefaults.standard.set(theme, forKey: "native.appearance")
                for width in [1400, 780] {
                    let window = try appWindow()
                    window.setContentSize(NSSize(width: width, height: 900))
                    window.center()
                    store.route = .machines
                    for layout in ["grid", "list"] {
                        UserDefaults.standard.set(layout, forKey: "native.machineLayout")
                        if layout == "grid" { try await previewCheck() }
                        try await Task.sleep(for: .milliseconds(300))
                        try await capture(folder.appendingPathComponent("\(theme)-\(width)-machines-\(layout).png"))
                    }
                }
            }
            store.route = .machine(id)
            let webView = try await shellWebView()
            let command = "printf '%s\\n' native-terminal-ok > /mnt/job/native-qa.txt"
            try await typeCommand(webView, text: command)
            let result = home.appendingPathComponent("jobs/\(id)/share/native-qa.txt")
            var passed = false
            for _ in 0..<100 {
                if (try? String(contentsOf: result)) == "native-terminal-ok\n" { passed = true; break }
                try await Task.sleep(for: .milliseconds(100))
            }
            guard passed else { throw RuntimeError.message("Native terminal input did not reach the VM.") }
            try await clipboardCheck(webView)
            try await capture(folder.appendingPathComponent("native-terminal.png"))
            _ = try await store.api.request("/api/tasks/\(id)/stop", method: "POST")
            for _ in 0..<300 {
                if try await store.api.request("/api/tasks/\(id)").isEnded { return }
                try await Task.sleep(for: .milliseconds(100))
            }
            throw RuntimeError.message("The verification VM did not finish stopping.")
        } catch {
            _ = try? await store.api.request("/api/tasks/\(id)/stop", method: "POST")
            throw error
        }
    }

    private static func previewCheck() async throws {
        for _ in 0..<100 {
            let stats = try? await evaluate(NativePreviewHub.shared.webView, "return window.nativePreview?.stats()")
            if let stats = stats as? [String: Any], let measured = stats["nonblank"] as? Int,
               let instances = stats["activeWasmInstances"] as? Int, measured > 0 && instances == 1 { return }
            try await Task.sleep(for: .milliseconds(100))
        }
        throw RuntimeError.message("The live VM preview did not render through the shared terminal core.")
    }

    private static func typeCommand(_ view: WKWebView, text: String) async throws {
        _ = try await evaluate(view, "window.nativeTerminal.paste(text); window.webkit.messageHandlers.nativeTerminal.postMessage({type: 'input', text: '\\r'});",
                               arguments: ["text": text])
    }

    private static func clipboardCheck(_ view: WKWebView) async throws {
        let clipboard = NSPasteboard.general
        let original = (clipboard.pasteboardItems ?? []).map { item in
            item.types.reduce(into: [NSPasteboard.PasteboardType: Data]()) { result, type in result[type] = item.data(forType: type) }
        }
        let marker = "native-clipboard-ok-\(UUID().uuidString)"
        defer {
            // Preserve a newer clipboard entry if the user copied something during the check.
            if clipboard.string(forType: .string) == marker {
                let restored = original.map { values in
                    let item = NSPasteboardItem()
                    for (type, data) in values { item.setData(data, forType: type) }
                    return item
                }
                clipboard.clearContents()
                clipboard.writeObjects(restored)
            }
        }
        let encoded = Data(marker.utf8).base64EncodedString()
        try await typeCommand(view, text: "printf '\\033]52;c;\(encoded)\\a'")
        for _ in 0..<100 {
            if clipboard.string(forType: .string) == marker { return }
            try await Task.sleep(for: .milliseconds(100))
        }
        throw RuntimeError.message("The real VM terminal did not deliver its clipboard escape sequence.")
    }

    private static func shellWebView() async throws -> WKWebView {
        for _ in 0..<200 {
            for view in webViews(try appWindow().contentView!) {
                guard view.identifier?.rawValue == "shell" else { continue }
                let ready = try? await evaluate(view, "return window.nativeTerminal?.ready === true")
                if ready as? Bool == true {
                    return view
                }
            }
            try await Task.sleep(for: .milliseconds(100))
        }
        throw RuntimeError.message("Native terminal renderer did not initialize.")
    }

    private static func webViews(_ view: NSView) -> [WKWebView] {
        if let view = view as? WKWebView { return [view] }
        return view.subviews.flatMap(webViews)
    }

    private static func evaluate(_ view: WKWebView, _ script: String, arguments: [String: Any] = [:]) async throws -> Any {
        try await withCheckedThrowingContinuation { continuation in
            view.callAsyncJavaScript(script, arguments: arguments, in: nil, in: .page) { result in
                continuation.resume(with: result)
            }
        }
    }

    private static func appWindow() throws -> NSWindow {
        guard let window = NSApp.windows.first(where: {
            $0.contentView != nil && $0.styleMask.contains(.titled) && $0.title.hasPrefix("agentvm Verification")
        }) else {
            throw RuntimeError.message("Native window unavailable.")
        }
        return window
    }

    private static func capture(_ destination: URL) async throws {
        if #available(macOS 14.4, *) {
            let content = try await SCShareableContent.currentProcess
            let window = try appWindow()
            guard let captured = content.windows.first(where: { $0.windowID == CGWindowID(window.windowNumber) }) else {
                throw RuntimeError.message("This app's window is unavailable for capture.")
            }
            let configuration = SCStreamConfiguration()
            configuration.width = Int(window.frame.width * window.backingScaleFactor)
            configuration.height = Int(window.frame.height * window.backingScaleFactor)
            let image: CGImage = try await withCheckedThrowingContinuation { continuation in
                SCScreenshotManager.captureImage(contentFilter: SCContentFilter(desktopIndependentWindow: captured), configuration: configuration) { image, error in
                    if let image { continuation.resume(returning: image) }
                    else { continuation.resume(throwing: error ?? RuntimeError.message("The native window could not be captured.")) }
                }
            }
            guard let png = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:]) else {
                throw RuntimeError.message("The native capture could not be saved.")
            }
            try png.write(to: destination)
            return
        }
        let view = try appWindow().contentView!
        view.layoutSubtreeIfNeeded()
        guard let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds) else {
            throw RuntimeError.message("The native view could not be captured.")
        }
        view.cacheDisplay(in: view.bounds, to: bitmap)
        guard let png = bitmap.representation(using: .png, properties: [:]) else {
            throw RuntimeError.message("The native capture could not be saved.")
        }
        try png.write(to: destination)
    }
}
