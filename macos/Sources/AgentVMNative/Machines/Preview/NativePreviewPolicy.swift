import Foundation

enum NativePreviewPolicy {
    static func request(baseURL: URL, id: String) -> URLRequest? {
        guard baseURL.scheme == "http", ["127.0.0.1", "localhost"].contains(baseURL.host ?? ""),
              baseURL.port != nil, baseURL.user == nil, baseURL.password == nil, !id.isEmpty,
              var parts = URLComponents(url: baseURL, resolvingAgainstBaseURL: false) else { return nil }
        parts.scheme = "ws"
        parts.path = "/api/tasks/\(id)/pty"
        parts.queryItems = [URLQueryItem(name: "session", value: "claude"), URLQueryItem(name: "view", value: "true"),
                            URLQueryItem(name: "cols", value: "200"), URLQueryItem(name: "rows", value: "60")]
        parts.fragment = nil
        guard let url = parts.url else { return nil }
        var request = URLRequest(url: url)
        request.setValue(baseURL.absoluteString, forHTTPHeaderField: "Origin")
        return request
    }

    static func shouldStream(visible: Bool, active: Bool, running: Bool, interactive: Bool, ready: Bool) -> Bool {
        visible && active && running && interactive && ready
    }
}

struct NativePreviewBuffer {
    private var bytes = Data()
    var isEmpty: Bool { bytes.isEmpty }

    mutating func append(_ data: Data) -> Bool {
        guard data.count <= (1 << 20) - bytes.count else { return false }
        bytes.append(data)
        return true
    }
    mutating func take() -> Data { let result = bytes; bytes = Data(); return result }
    mutating func clear() { bytes = Data() }
}
