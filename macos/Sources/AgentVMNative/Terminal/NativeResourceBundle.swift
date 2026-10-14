import Foundation

enum NativeResourceBundle {
    static func resources(in application: Bundle) -> URL? {
        let embedded = application.url(forResource: "AgentVMNative_AgentVMNative", withExtension: "bundle")
            .flatMap(Bundle.init(url:))
        // Packaged apps must never depend on the original machine's SwiftPM build directory.
        if application.bundleURL.pathExtension == "app", embedded == nil { return nil }
        let bundle = embedded ?? Bundle.module
        // SwiftPM's native and swiftbuild backends produce different resource directory layouts.
        return (bundle.url(forResource: "terminal", withExtension: "html", subdirectory: "Resources")
            ?? bundle.url(forResource: "terminal", withExtension: "html"))?.deletingLastPathComponent()
    }

    static func verify(in application: Bundle) throws {
        guard let root = resources(in: application) else {
            throw RuntimeError.message("The native app's embedded terminal resources are missing.")
        }
        for path in ["terminal.html", "preview.html", "dist/terminal.js", "dist/preview.js", "dist/terminal.css"] {
            let bytes = try Data(contentsOf: root.appendingPathComponent(path), options: .mappedIfSafe)
            guard !bytes.isEmpty else {
                throw RuntimeError.message("The native app's embedded resource is empty: \(path).")
            }
        }
    }
}
