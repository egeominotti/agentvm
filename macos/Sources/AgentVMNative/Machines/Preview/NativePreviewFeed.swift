import Combine
import Foundation

@MainActor
final class NativePreviewFeed: ObservableObject {
    @Published private(set) var screen = NativePreviewFrame.empty
    @Published private(set) var message = "Opening the live Claude terminal…"
    private let api: NativeAPI
    private let id: String
    private let hub = NativePreviewHub.shared
    private var visible = false
    private var active = false
    private var live = false
    private var key: String?
    private var socket: URLSessionWebSocketTask?
    private var reader: Task<Void, Never>?
    private var setup: Task<Void, Never>?
    private var flush: Task<Void, Never>?
    private var retry: Task<Void, Never>?
    private var buffer = NativePreviewBuffer()

    init(api: NativeAPI, id: String) { self.api = api; self.id = id }

    func setVisible(_ value: Bool) { visible = value; follow() }
    func setActive(_ value: Bool) { active = value; follow() }
    func setLive(_ value: Bool) { live = value; follow() }
    private var wanted: Bool { visible && active && live }

    private func follow() {
        if !wanted { stop(); return }
        guard key == nil, retry == nil else { return }
        let next = UUID().uuidString
        key = next
        hub.attach(next, feed: self)
    }

    func parserReady(_ expected: String) {
        guard key == expected, wanted, setup == nil, socket == nil else { return }
        setup = Task { [weak self] in
            guard let self else { return }
            do {
                try await hub.create(expected)
                guard !Task.isCancelled, key == expected, wanted else { return }
                setup = nil
                connect(expected)
            } catch {
                if !Task.isCancelled, key == expected { reconnect(expected) }
            }
        }
    }

    private func connect(_ expected: String) {
        guard let request = NativePreviewPolicy.request(baseURL: api.baseURL, id: id) else { parserUnavailable(expected); return }
        let connection = URLSession.shared.webSocketTask(with: request)
        socket = connection
        connection.resume()
        reader = Task { [weak self] in
            do {
                while !Task.isCancelled {
                    let received = try await connection.receive()
                    guard let self, key == expected else { return }
                    let bytes: Data
                    switch received {
                    case .data(let data): bytes = data
                    case .string(let text): bytes = Data(text.utf8)
                    @unknown default: continue
                    }
                    guard buffer.append(bytes) else { reconnect(expected); return }
                    scheduleFrame(expected)
                }
            } catch {
                guard let self, !Task.isCancelled, key == expected else { return }
                reconnect(expected)
            }
        }
    }

    private func scheduleFrame(_ expected: String) {
        guard flush == nil, !buffer.isEmpty else { return }
        flush = Task { [weak self] in
            do {
                try await Task.sleep(for: .milliseconds(200))
                guard let self, key == expected, wanted else { return }
                let frame = try await hub.frame(expected, bytes: buffer.take())
                guard !Task.isCancelled, key == expected, wanted else { return }
                if screen != frame { screen = frame }
                flush = nil
                scheduleFrame(expected)
            } catch {
                guard let self, !Task.isCancelled, key == expected else { return }
                reconnect(expected)
            }
        }
    }

    private func reconnect(_ expected: String) {
        guard key == expected else { return }
        stop()
        message = "Reconnecting to the live terminal…"
        retry = Task { [weak self] in
            do { try await Task.sleep(for: .milliseconds(900)) } catch { return }
            guard let self else { return }
            retry = nil
            follow()
        }
    }

    func parserUnavailable(_ expected: String) {
        guard key == expected else { return }
        stop()
        message = "The live terminal preview is unavailable."
    }

    func parserRestarted(_ expected: String) {
        guard key == expected else { return }
        stop()
        follow()
    }

    private func stop() {
        setup?.cancel(); setup = nil
        reader?.cancel(); reader = nil
        flush?.cancel(); flush = nil
        retry?.cancel(); retry = nil
        socket?.cancel(with: .goingAway, reason: nil); socket = nil
        buffer.clear()
        if let key { hub.detach(key) }
        key = nil
    }
}
