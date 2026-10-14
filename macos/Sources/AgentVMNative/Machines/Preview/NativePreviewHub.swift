import Foundation
import WebKit

struct NativePreviewFrame: Codable, Equatable, Sendable {
    let text: String
    let cols: Int
    let rows: Int
    static let empty = NativePreviewFrame(text: "", cols: 60, rows: 16)
}

@MainActor
final class NativePreviewHub: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
    static let shared = NativePreviewHub()
    private final class Client {
        weak var feed: NativePreviewFeed?
        init(_ feed: NativePreviewFeed) { self.feed = feed }
    }
    private var clients: [String: Client] = [:]
    private var ready = false
    lazy var webView: WKWebView = {
        let configuration = WKWebViewConfiguration()
        configuration.setURLSchemeHandler(TerminalAssets(), forURLScheme: "agentvm-terminal")
        configuration.userContentController.add(self, name: "nativePreview")
        let view = WKWebView(frame: .zero, configuration: configuration)
        view.navigationDelegate = self
        load(view)
        return view
    }()

    func attach(_ key: String, feed: NativePreviewFeed) {
        clients[key] = Client(feed)
        _ = webView
        if ready { feed.parserReady(key) }
    }

    func detach(_ key: String) {
        clients.removeValue(forKey: key)
        guard ready else { return }
        webView.callAsyncJavaScript("window.nativePreview.remove(key)", arguments: ["key": key], in: nil, in: .page, completionHandler: nil)
    }

    func create(_ key: String) async throws {
        guard ready, clients[key]?.feed != nil else { throw CancellationError() }
        _ = try await call("return window.nativePreview.create(key)", arguments: ["key": key])
    }

    func frame(_ key: String, bytes: Data) async throws -> NativePreviewFrame {
        guard ready, clients[key]?.feed != nil else { throw CancellationError() }
        let result = try await call("return window.nativePreview.frame(key, bytes)", arguments: ["key": key, "bytes": bytes.base64EncodedString()])
        let data = try JSONSerialization.data(withJSONObject: result)
        let frame = try JSONDecoder().decode(NativePreviewFrame.self, from: data)
        guard (60...200).contains(frame.cols), (16...60).contains(frame.rows), frame.text.utf8.count <= 1 << 20 else {
            throw NativeAPIError(status: nil, message: "The terminal preview is larger than its display budget.")
        }
        return frame
    }

    private func call(_ script: String, arguments: [String: Any]) async throws -> Any {
        try await withCheckedThrowingContinuation { continuation in
            webView.callAsyncJavaScript(script, arguments: arguments, in: nil, in: .page) { continuation.resume(with: $0) }
        }
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.frameInfo.isMainFrame, message.frameInfo.request.url?.scheme == "agentvm-terminal",
              let body = message.body as? [String: Any], let type = body["type"] as? String else { return }
        if type == "ready" {
            ready = true
            for (key, client) in clients { client.feed?.parserReady(key) }
        } else if type == "error" {
            for (key, client) in clients { client.feed?.parserUnavailable(key) }
        }
    }

    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        ready = false
        let previous = clients
        clients.removeAll()
        for (key, client) in previous { client.feed?.parserRestarted(key) }
        load(webView)
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        for (key, client) in clients { client.feed?.parserUnavailable(key) }
    }

    private func load(_ view: WKWebView) {
        view.load(URLRequest(url: URL(string: "agentvm-terminal://view/preview.html")!))
    }
}
