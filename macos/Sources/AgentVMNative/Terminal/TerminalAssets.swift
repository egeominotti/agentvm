import Foundation
import WebKit

final class TerminalAssets: NSObject, WKURLSchemeHandler {
    private let root = Bundle.module.resourceURL!.appendingPathComponent("Resources", isDirectory: true)

    func webView(_ webView: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        guard let url = urlSchemeTask.request.url else { return }
        let relative = url.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        let file = root.appendingPathComponent(relative).standardizedFileURL
        guard file.path.hasPrefix(root.path + "/"), let bytes = try? Data(contentsOf: file) else {
            urlSchemeTask.didFailWithError(URLError(.fileDoesNotExist))
            return
        }
        let type: String
        switch file.pathExtension {
        case "html": type = "text/html"
        case "js": type = "application/javascript"
        case "css": type = "text/css"
        case "ttf": type = "font/ttf"
        case "woff2": type = "font/woff2"
        case "wasm": type = "application/wasm"
        default: type = "application/octet-stream"
        }
        let response = HTTPURLResponse(url: url, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: [
            "Content-Type": type, "Access-Control-Allow-Origin": "*", "Cache-Control": "no-cache"
        ])!
        urlSchemeTask.didReceive(response)
        urlSchemeTask.didReceive(bytes)
        urlSchemeTask.didFinish()
    }

    func webView(_ webView: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {}
}
