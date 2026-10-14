import SwiftUI
import WebKit

struct NativePreviewRuntimeView: NSViewRepresentable {
    func makeNSView(context: Context) -> WKWebView { NativePreviewHub.shared.webView }
    func updateNSView(_ view: WKWebView, context: Context) { }
}
