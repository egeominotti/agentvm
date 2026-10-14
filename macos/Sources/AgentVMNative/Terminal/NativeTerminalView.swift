import AppKit
import SwiftUI
import WebKit

struct NativeTerminalView: NSViewRepresentable {
    let store: NativeStore
    let id: String
    let session: String
    let live: Bool
    @Environment(\.colorScheme) private var colorScheme

    func makeCoordinator() -> TerminalCoordinator { TerminalCoordinator(store: store, id: id, session: session) }

    func makeNSView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.setURLSchemeHandler(TerminalAssets(), forURLScheme: "agentvm-terminal")
        configuration.userContentController.add(context.coordinator, name: "nativeTerminal")
        let view = WKWebView(frame: .zero, configuration: configuration)
        view.identifier = NSUserInterfaceItemIdentifier(session)
        view.setAccessibilityLabel("\(NativeMachineLogic.shellLabel(session)) terminal")
        view.navigationDelegate = context.coordinator
        context.coordinator.webView = view
        context.coordinator.setLive(live)
        let theme = colorScheme == .dark ? "dark" : "light"
        context.coordinator.setTheme(theme)
        view.load(URLRequest(url: URL(string: "agentvm-terminal://view/terminal.html?theme=\(theme)")!))
        return view
    }

    func updateNSView(_ view: WKWebView, context: Context) {
        context.coordinator.setLive(live)
        context.coordinator.setTheme(colorScheme == .dark ? "dark" : "light")
    }

    static func dismantleNSView(_ view: WKWebView, coordinator: TerminalCoordinator) {
        coordinator.close()
        view.configuration.userContentController.removeScriptMessageHandler(forName: "nativeTerminal")
        view.navigationDelegate = nil
    }
}
