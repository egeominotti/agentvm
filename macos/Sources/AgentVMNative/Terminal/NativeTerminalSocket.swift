import Foundation

@MainActor
final class NativeTerminalSocket: NSObject, URLSessionWebSocketDelegate {
    private var session: URLSession!
    private var request: URLRequest?
    private var socket: URLSessionWebSocketTask?
    private var reader: Task<Void, Never>?
    private var writer: Task<Void, Never>?
    private var retry: Task<Void, Never>?
    private var state = NativeTerminalSocketState()
    private var active = false
    private var closed = false
    private var writerID: UUID?
    private let onStatus: (Bool) -> Void
    private let onOutput: (Data) -> Void

    init(onStatus: @escaping (Bool) -> Void, onOutput: @escaping (Data) -> Void) {
        self.onStatus = onStatus; self.onOutput = onOutput
        super.init()
        session = URLSession(configuration: .ephemeral, delegate: self, delegateQueue: .main)
    }

    func connect(_ request: URLRequest) {
        guard !closed else { return }
        self.request = request
        active = true
        open()
    }

    @discardableResult
    func input(_ data: Data) -> Bool {
        guard state.enqueueInput(data) else { return false }
        drain()
        return true
    }

    func resize(cols: Int, rows: Int) {
        state.resize(cols: cols, rows: rows)
        if let url = request?.url, var parts = URLComponents(url: url, resolvingAgainstBaseURL: false) {
            parts.queryItems = (parts.queryItems ?? []).filter { !["cols", "rows"].contains($0.name) }
                + [URLQueryItem(name: "cols", value: String(cols)), URLQueryItem(name: "rows", value: String(rows))]
            request?.url = parts.url
        }
        drain()
    }

    func pause() {
        active = false
        disconnect()
    }

    func close() {
        closed = true
        pause()
        state.close()
        session.invalidateAndCancel()
    }

    private func open() {
        guard active, !closed, socket == nil, let request else { return }
        let socket = session.webSocketTask(with: request)
        self.socket = socket
        socket.resume()
        reader = Task { [weak self] in
            do {
                while !Task.isCancelled {
                    let message = try await socket.receive()
                    guard let self, self.socket === socket, !Task.isCancelled else { return }
                    switch message {
                    case .data(let data): self.onOutput(data)
                    case .string(let text): self.onOutput(Data(text.utf8))
                    @unknown default: continue
                    }
                }
            } catch {
                guard let self, !Task.isCancelled else { return }
                self.failed(socket)
            }
        }
    }

    private func drain() {
        guard active, state.opened, let socket, writer == nil, state.hasPending else { return }
        let token = UUID()
        writerID = token
        writer = Task { [weak self] in
            defer {
                if let self, self.writerID == token { self.writer = nil; self.writerID = nil }
            }
            do {
                while !Task.isCancelled {
                    guard let self, self.socket === socket, let next = self.state.nextAttempt() else { return }
                    let message: URLSessionWebSocketTask.Message
                    switch next {
                    case .input(let data): message = .data(data)
                    case .resize(let cols, let rows):
                        let data = try JSONSerialization.data(withJSONObject: ["cols": cols, "rows": rows])
                        message = .string(String(decoding: data, as: UTF8.self))
                    }
                    try await socket.send(message)
                }
            } catch {
                guard let self, !Task.isCancelled else { return }
                self.failed(socket)
            }
        }
    }

    private func disconnect() {
        retry?.cancel(); retry = nil
        reader?.cancel(); reader = nil
        writer?.cancel(); writer = nil; writerID = nil
        let previous = socket
        socket = nil
        state.connectionClosed()
        previous?.cancel(with: .goingAway, reason: nil)
        onStatus(false)
    }

    private func failed(_ failedSocket: URLSessionWebSocketTask) {
        guard socket === failedSocket else { return }
        disconnect()
        guard active, !closed else { return }
        retry = Task { [weak self] in
            do { try await Task.sleep(for: .milliseconds(900)) } catch { return }
            guard let self, !Task.isCancelled else { return }
            self.retry = nil
            self.open()
        }
    }

    nonisolated func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask,
                               didOpenWithProtocol protocol: String?) {
        Task { @MainActor [weak self] in
            guard let self, self.socket === webSocketTask, self.active, !self.closed else { return }
            self.state.connectionOpened()
            self.onStatus(true)
            self.drain()
        }
    }

    nonisolated func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask,
                               didCloseWith closeCode: URLSessionWebSocketTask.CloseCode, reason: Data?) {
        Task { @MainActor [weak self] in self?.failed(webSocketTask) }
    }
}
