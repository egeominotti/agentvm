import Foundation

struct NativeAPIError: LocalizedError, Sendable {
    let status: Int?
    let message: String
    var errorDescription: String? { message }
    var unconfiguredBackup: Bool { status == 400 && message.localizedCaseInsensitiveContains("not configured") }
}

struct NativeAPI: Sendable {
    let baseURL: URL
    private let session: URLSession

    init(baseURL: URL) {
        self.baseURL = baseURL
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 360
        configuration.timeoutIntervalForResource = 3600
        configuration.httpShouldSetCookies = false
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        self.session = URLSession(configuration: configuration)
    }

    func url(_ path: String) -> URL {
        let requested = path.hasPrefix("/") ? path : "/" + path
        let candidate = URL(string: requested, relativeTo: baseURL)?.absoluteURL ?? baseURL
        var components = URLComponents(url: candidate, resolvingAgainstBaseURL: true)!
        // A path or query received from the server must never redirect a native action to
        // another origin. Terminals derive their WebSocket URL from this same server URL.
        components.scheme = baseURL.scheme
        components.host = baseURL.host
        components.port = baseURL.port
        components.user = nil
        components.password = nil
        components.fragment = nil
        return components.url ?? baseURL
    }

    func request(_ path: String, method: String = "GET", body: JSON? = nil) async throws -> JSON {
        var request = try makeRequest(path, method: method)
        if let body {
            request.httpBody = try JSONEncoder().encode(body)
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        let (data, response) = try await transfer { try await session.data(for: request) }
        try validate(response, data: data)
        return try decode(data)
    }

    func text(_ path: String) async throws -> String {
        let request = try makeRequest(path, method: "GET")
        let (data, response) = try await transfer { try await session.data(for: request) }
        try validate(response, data: data)
        guard let text = String(data: data, encoding: .utf8) else {
            throw NativeAPIError(status: nil, message: "The server returned text that cannot be read.")
        }
        return text
    }

    func events(_ path: String, receive: @Sendable (NativeSSERecord) async -> Bool) async throws {
        var request = try makeRequest(path, method: "GET")
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        request.timeoutInterval = 60
        try await transfer {
            let (bytes, response) = try await session.bytes(for: request)
            defer { bytes.task.cancel() }
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            if !(200..<300).contains(status) {
                var errorBody = Data()
                for try await byte in bytes {
                    errorBody.append(byte)
                    if errorBody.count >= 64 << 10 { break }
                }
                try validate(response, data: errorBody)
            }
            try validate(response, data: Data())
            let contentType = (response as? HTTPURLResponse)?.value(forHTTPHeaderField: "Content-Type") ?? ""
            guard contentType.lowercased().hasPrefix("text/event-stream") else {
                throw NativeAPIError(status: nil, message: "The server did not return an event stream.")
            }
            // AsyncBytes.lines can allocate an unbounded line before the caller sees it.
            var decoder = NativeSSEDecoder()
            for try await byte in bytes {
                try Task.checkCancellation()
                if let record = try decoder.append(byte), !(await receive(record)) { return }
            }
        }
    }

    func upload(_ path: String, file: URL, headers: [String: String] = [:]) async throws -> JSON {
        var request = try makeRequest(path, method: "POST")
        request.setValue("application/octet-stream", forHTTPHeaderField: "Content-Type")
        for (name, value) in headers { request.setValue(value, forHTTPHeaderField: name) }
        // URLSession streams from the file rather than loading a VM archive into RAM.
        let (data, response) = try await transfer { try await session.upload(for: request, fromFile: file) }
        try validate(response, data: data)
        return try decode(data)
    }

    func download(_ path: String, to destination: URL) async throws {
        let request = try makeRequest(path, method: "GET")
        let (temporary, response) = try await transfer { try await session.download(for: request) }
        defer { try? FileManager.default.removeItem(at: temporary) }
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        if !(200..<300).contains(status) {
            let file = try FileHandle(forReadingFrom: temporary)
            defer { try? file.close() }
            try validate(response, data: try file.read(upToCount: 64 << 10) ?? Data())
        }
        let manager = FileManager.default
        if manager.fileExists(atPath: destination.path) {
            _ = try manager.replaceItemAt(destination, withItemAt: temporary)
        } else {
            try manager.moveItem(at: temporary, to: destination)
        }
    }

    private func transfer<T>(_ work: () async throws -> T) async throws -> T {
        do {
            try Task.checkCancellation()
            let result = try await work()
            try Task.checkCancellation()
            return result
        } catch {
            // URLSession reports normal task cancellation as URLError.cancelled. Views and
            // stores already handle CancellationError without showing an error to the user.
            if Task.isCancelled || (error as? URLError)?.code == .cancelled { throw CancellationError() }
            throw error
        }
    }

    private func makeRequest(_ path: String, method: String) throws -> URLRequest {
        guard baseURL.scheme == "http", ["127.0.0.1", "localhost"].contains(baseURL.host ?? ""),
              baseURL.user == nil, baseURL.password == nil, baseURL.port != nil else {
            throw NativeAPIError(status: nil, message: "Connect to a local agentvm server such as http://127.0.0.1:7777.")
        }
        var request = URLRequest(url: url(path))
        request.httpMethod = method.uppercased()
        request.timeoutInterval = NativeRequestPolicy.timeout(path: request.url?.path ?? path, method: method)
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        return request
    }

    private func validate(_ response: URLResponse, data: Data) throws {
        guard let response = response as? HTTPURLResponse else {
            throw NativeAPIError(status: nil, message: "The server returned an unexpected response.")
        }
        guard (200..<300).contains(response.statusCode) else {
            let detail = (try? JSONDecoder().decode(JSON.self, from: data))?["error"].text ?? ""
            let fallback = HTTPURLResponse.localizedString(forStatusCode: response.statusCode)
            throw NativeAPIError(status: response.statusCode, message: detail.isEmpty ? fallback : detail)
        }
    }

    private func decode(_ data: Data) throws -> JSON {
        if data.isEmpty { return .null }
        do { return try JSONDecoder().decode(JSON.self, from: data) }
        catch { throw NativeAPIError(status: nil, message: "The server returned an unreadable JSON response.") }
    }
}
