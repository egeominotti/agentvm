import Foundation

enum NativeMCPPolicy {
    static func configuration(server: URL, endpoint: URL) -> JSON? {
        guard server.isFileURL, let parts = URLComponents(url: endpoint, resolvingAgainstBaseURL: false),
              parts.scheme == "http", ["127.0.0.1", "localhost"].contains(parts.host ?? ""),
              let port = parts.port, (1...65535).contains(port), parts.user == nil, parts.password == nil,
              parts.query == nil, parts.fragment == nil, ["", "/"].contains(parts.path) else { return nil }
        return .object(["mcpServers": .object(["agentvm": .object([
            "command": .string(server.path), "args": .array([]),
            "env": .object(["AGENTVM_URL": .string(endpoint.absoluteString)])
        ])])])
    }
}
