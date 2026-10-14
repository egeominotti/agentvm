import AppKit
import Foundation
import WebKit

@MainActor
final class TerminalCoordinator: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
    weak var webView: WKWebView?
    var live = true
    private let store: NativeStore
    private let id: String
    private let session: String
    private lazy var transport = NativeTerminalSocket(onStatus: { [weak self] connected in
        self?.evaluate("window.nativeTerminal?.state(connected)", arguments: ["connected": connected])
    }, onOutput: { [weak self] data in
        self?.evaluate("window.nativeTerminal?.output(bytes)", arguments: ["bytes": data.base64EncodedString()])
    })
    private var pasteObserver: NSObjectProtocol?
    private var selectionObserver: NSObjectProtocol?
    private var keyObserver: NSObjectProtocol?
    private var cols = 120
    private var rows = 36
    private var ready = false
    private var rendering = NativeTerminalState()
    private var theme = "light"

    init(store: NativeStore, id: String, session: String) {
        self.store = store; self.id = id; self.session = session
        super.init()
        pasteObserver = NotificationCenter.default.addObserver(forName: Notification.Name("AgentVMNativePaste"), object: nil, queue: .main) { [weak self] note in
            guard let info = note.userInfo as? [String: String] else { return }
            Task { @MainActor in
                guard let self, info["id"] == self.id, info["session"] == self.session else { return }
                self.paste(info["text"] ?? "")
            }
        }
        selectionObserver = NotificationCenter.default.addObserver(forName: Notification.Name("AgentVMNativeSessionSelected"), object: nil, queue: .main) { [weak self] note in
            guard let info = note.userInfo as? [String: String] else { return }
            Task { @MainActor in
                guard let self, info["id"] == self.id, info["session"] == self.session else { return }
                self.focusSelected()
            }
        }
        keyObserver = NotificationCenter.default.addObserver(forName: NSWindow.didBecomeKeyNotification, object: nil, queue: .main) { [weak self] note in
            let window = note.object as? NSWindow
            Task { @MainActor in
                guard let self, let window, self.webView?.window === window else { return }
                self.focusSelected(preservingNativeInput: true)
            }
        }
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.frameInfo.isMainFrame, message.frameInfo.request.url?.scheme == "agentvm-terminal",
              let body = message.body as? [String: Any], let type = body["type"] as? String else { return }
        switch type {
        case "connect":
            ready = true
            size(body)
            connect()
        case "resize":
            size(body)
            transport.resize(cols: cols, rows: rows)
        case "input":
            if live, let text = body["text"] as? String, !transport.input(Data(text.utf8)) {
                store.error = "The terminal is reconnecting and its input buffer is full. Wait for it to reconnect, then try again."
            }
        case "copy":
            if let text = body["text"] as? String {
                NSPasteboard.general.clearContents()
                if NSPasteboard.general.setString(text, forType: .string) { store.notice = "Copied to the clipboard" }
                else { store.error = "Could not copy the terminal selection." }
            }
        case "paste":
            if let text = NSPasteboard.general.string(forType: .string) {
                paste(text)
            }
        case "ready":
            perform(rendering.rendererReady())
            focusSelected()
        case "error": store.error = body["message"] as? String
        default: break
        }
    }

    private func size(_ body: [String: Any]) {
        if let value = body["cols"] as? Int, value >= 20 { cols = min(value, 65535) }
        if let value = body["rows"] as? Int, value >= 5 { rows = min(value, 65535) }
    }

    private func connect() {
        guard live, ready else { return }
        var components = URLComponents(url: store.api.url("/api/tasks/\(id)/pty"), resolvingAgainstBaseURL: false)!
        components.scheme = "ws"
        components.queryItems = [URLQueryItem(name: "session", value: session), URLQueryItem(name: "cols", value: String(cols)),
                                 URLQueryItem(name: "rows", value: String(rows))]
        var request = URLRequest(url: components.url!)
        request.setValue(store.api.baseURL.absoluteString, forHTTPHeaderField: "Origin")
        transport.resize(cols: cols, rows: rows)
        transport.connect(request)
    }

    func evaluate(_ script: String, arguments: [String: Any] = [:]) {
        webView?.callAsyncJavaScript(script, arguments: arguments, in: nil, in: .page, completionHandler: nil)
    }

    func setTheme(_ value: String) {
        theme = value == "dark" ? "dark" : "light"
        perform(rendering.setTheme(theme))
    }

    private func paste(_ text: String) {
        guard live else { return }
        perform(rendering.paste(text))
    }

    private func perform(_ commands: [NativeTerminalCommand]) {
        for command in commands {
            switch command {
            case .theme(let theme): evaluate("window.nativeTerminal.theme(theme)", arguments: ["theme": theme])
            case .paste(let text):
                focusSelected()
                evaluate("window.nativeTerminal.paste(text)", arguments: ["text": text])
            case .pasteRejected: store.error = "The terminal is still opening and its input buffer is full. Paste again once it is ready."
            }
        }
    }

    func close() {
        live = false
        transport.close()
        if let pasteObserver { NotificationCenter.default.removeObserver(pasteObserver); self.pasteObserver = nil }
        if let selectionObserver { NotificationCenter.default.removeObserver(selectionObserver); self.selectionObserver = nil }
        if let keyObserver { NotificationCenter.default.removeObserver(keyObserver); self.keyObserver = nil }
    }

    private func focusSelected(preservingNativeInput: Bool = false) {
        let active = UserDefaults.standard.string(forKey: "native.activeSession.\(id)") ?? "claude"
        guard store.route == .machine(id), active == session, let webView, webView.window?.isKeyWindow == true else { return }
        if preservingNativeInput, let responder = webView.window?.firstResponder,
           responder is NSText || responder is NSControl { return }
        webView.window?.makeFirstResponder(webView)
        evaluate("window.nativeTerminal?.focus()")
    }

    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        transport.pause()
        ready = false
        rendering.rendererReset()
        // Recover only this renderer. The VM and its tmux session continue independently.
        webView.load(URLRequest(url: URL(string: "agentvm-terminal://view/terminal.html?theme=\(theme)")!))
    }

    func setLive(_ value: Bool) {
        live = value
        if value { connect(); return }
        transport.pause()
    }

    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction,
                 decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        guard let url = navigationAction.request.url else { decisionHandler(.cancel); return }
        if url.scheme == "agentvm-terminal" { decisionHandler(.allow) }
        else {
            if ["http", "https"].contains(url.scheme ?? "") { NSWorkspace.shared.open(url) }
            decisionHandler(.cancel)
        }
    }
}
