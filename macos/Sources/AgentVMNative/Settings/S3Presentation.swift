import Foundation

enum NativeS3Presentation {
    static func message(raw: String, endpoint: String) -> String {
        let lower = raw.lowercased()
        if lower.contains("invalidaccesskeyid") || lower.contains("signaturedoesnotmatch") {
            return "S3 rejected the access key or secret. Check the saved credentials for this provider."
        }
        if lower.contains("accessdenied") {
            return "S3 denied access to the bucket. Check the access key, secret and bucket permissions."
        }
        if lower.contains("nosuchbucket") {
            return "The S3 bucket could not be found. Check the bucket name and region."
        }
        let connectionFailure = ["failed to connect", "couldn't connect", "could not connect", "connection refused",
                                 "could not resolve host", "connection timed out", "curl: (7)", "curl: (6)"]
            .contains { lower.contains($0) }
        if connectionFailure {
            var components = URLComponents(string: endpoint)
            components?.user = nil
            components?.password = nil
            components?.query = nil
            components?.fragment = nil
            let address = components?.string ?? endpoint
            let location = address.isEmpty ? "" : " at \(address)"
            let local = ["localhost", "127.0.0.1", "::1"].contains(components?.host?.lowercased() ?? "")
            let next = local ? "Start the local S3 service, then retry." : "Check the endpoint and network, then retry."
            return "The S3 server\(location) is not reachable. \(next)"
        }
        // curl retries may repeat the same diagnostic. Keep the server's useful detail once.
        var seen = Set<String>()
        return raw.components(separatedBy: .newlines)
            .filter { !$0.isEmpty && seen.insert($0).inserted }.joined(separator: "\n")
    }

    static func status(configured: Bool, checking: Bool, verified: Bool, failure: String?) -> String {
        if checking { return "Testing…" }
        if failure != nil { return "Unavailable" }
        if verified { return "Connected" }
        return configured ? "Saved; not checked" : "Not set up"
    }
}
