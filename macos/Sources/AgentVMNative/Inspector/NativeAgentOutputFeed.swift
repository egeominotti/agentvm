import Combine
import Foundation

@MainActor
final class NativeAgentOutputFeed: ObservableObject {
    @Published private(set) var entries: [NativeAgentOutputEntry] = []
    @Published private(set) var state = ""
    @Published private(set) var dropped = 0
    @Published private(set) var loaded = false
    @Published private(set) var connected = false
    @Published private(set) var error: String?
    private let api: NativeAPI
    private var buffer = NativeAgentOutputBuffer()
    private var generation = UUID()
    private var currentID = ""

    init(api: NativeAPI) { self.api = api }

    func run(id: String, ended: Bool) async {
        let token = UUID()
        generation = token
        if currentID != id {
            currentID = id
            buffer = NativeAgentOutputBuffer()
            entries = []; state = ""; dropped = 0; loaded = false
        }
        defer { if generation == token { connected = false } }
        repeat {
            do {
                try await read(id: id, ended: ended, token: token)
                guard !Task.isCancelled, generation == token else { return }
                if ended { return }
                error = "The event stream closed. Reconnecting…"
            } catch is CancellationError { return }
            catch {
                guard !Task.isCancelled, generation == token else { return }
                self.error = error.localizedDescription
            }
            connected = false
            do { try await Task.sleep(for: .milliseconds(900)) } catch { return }
        } while !Task.isCancelled && generation == token
    }

    private func read(id: String, ended: Bool, token: UUID) async throws {
        if ended {
            do { try await readCompleted(id: id, token: token); return }
            catch { if !NativeOutputPage.usesLegacyStream(error) { throw error } }
        }
        let channel = NativeAgentOutputChannel(buffer: buffer)
        let api = api
        try await withThrowingTaskGroup(of: Void.self) { group in
            group.addTask {
                try await api.events("/api/tasks/\(id)/events") { await channel.receive($0, ended: ended) }
            }
            group.addTask { [weak self] in
                while !Task.isCancelled {
                    try await Task.sleep(for: .milliseconds(200))
                    if let snapshot = await channel.snapshot() { await self?.apply(snapshot, token: token) }
                }
            }
            do { _ = try await group.next(); group.cancelAll() }
            catch { group.cancelAll(); throw error }
        }
        // Final delivery is bounded by the same interval as live delivery.
        try await Task.sleep(for: .milliseconds(200))
        if let snapshot = await channel.snapshot(force: true) { apply(snapshot, token: token) }
    }

    private func readCompleted(id: String, token: UUID) async throws {
        var history = buffer
        history.beginConnection()
        var cursor: UInt64 = 0
        while true {
            try Task.checkCancellation()
            guard generation == token else { throw CancellationError() }
            let data = try await api.request("/api/tasks/\(id)/output?cursor=\(cursor)&limit=100")
            let page = try NativeOutputPage(data, cursor: cursor)
            _ = history.append(page)
            if !page.more {
                apply(NativeAgentOutputSnapshot(buffer: history, loaded: true), token: token)
                return
            }
            cursor = page.nextCursor
        }
    }

    private func apply(_ snapshot: NativeAgentOutputSnapshot, token: UUID) {
        guard !Task.isCancelled, generation == token else { return }
        buffer = snapshot.buffer
        let next = buffer.entries
        if entries != next { entries = next }
        if state != buffer.state { state = buffer.state }
        if dropped != buffer.dropped { dropped = buffer.dropped }
        loaded = snapshot.loaded
        connected = snapshot.loaded
        if snapshot.loaded { error = nil }
    }
}
