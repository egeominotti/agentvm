import Foundation

enum NativeRequestPolicy {
    static func timeout(path: String, method: String) -> TimeInterval {
        let export = path.hasPrefix("/api/snapshots/") && path.hasSuffix("/export")
        return method.uppercased() == "GET" && !export ? 30 : 360
    }
}
