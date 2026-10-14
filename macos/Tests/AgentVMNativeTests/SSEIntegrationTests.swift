import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native actual event streaming", .serialized)
struct SSEIntegrationTests {
    private static var endpoint: URL? {
        guard let value = ProcessInfo.processInfo.environment["AGENTVM_NATIVE_SSE_TEST_URL"], let url = URL(string: value),
              url.scheme == "http", url.host == "127.0.0.1", let port = url.port, port != 7777 else { return nil }
        return url
    }
    private static var taskID: String? { ProcessInfo.processInfo.environment["AGENTVM_NATIVE_SSE_TASK_ID"] }

    @Test(.enabled(if: endpoint != nil && taskID != nil), .timeLimit(.minutes(1)))
    func realHistoryDrainsBeforeTheFirstServerKeepalive() async throws {
        let api = NativeAPI(baseURL: try #require(Self.endpoint))
        let id = try #require(Self.taskID)
        let collected = SSECollection()
        try await api.events("/api/tasks/\(id)/events") { await collected.receive($0, stopAtComment: true) }
        let (events, comments) = await collected.snapshot()
        #expect(!events.isEmpty)
        #expect(events.first?.type == "state")
        #expect(events.first?.data["state"].text == "queued")
        #expect(comments == 1)
        #expect(events.compactMap { UInt64($0.id) } == events.compactMap { UInt64($0.id) }.sorted())
    }

    @Test(.enabled(if: endpoint != nil && taskID != nil), .timeLimit(.minutes(1)))
    func cancellingARealOpenStreamStopsWithoutAUserFacingFailure() async throws {
        let api = NativeAPI(baseURL: try #require(Self.endpoint))
        let collected = SSECollection()
        let id = try #require(Self.taskID)
        let reading = Task { try await api.events("/api/tasks/\(id)/events") { await collected.receive($0) } }
        defer { reading.cancel() }
        for _ in 0..<300 {
            if await collected.hasEvents { break }
            if reading.isCancelled { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        let hasEvents = await collected.hasEvents
        try #require(hasEvents)
        let start = ContinuousClock.now
        reading.cancel()
        do { try await reading.value; Issue.record("Cancelled event streaming returned without CancellationError.") }
        catch is CancellationError { }
        #expect(start.duration(to: .now) < .seconds(2))
    }

    @Test(.enabled(if: endpoint != nil))
    func cancellationBeforeConnectingRemainsCancellation() async throws {
        let api = NativeAPI(baseURL: try #require(Self.endpoint))
        let reading = Task {
            withUnsafeCurrentTask { $0?.cancel() }
            do { try await api.events("/api/tasks/invalid/events") { _ in false }; return false }
            catch is CancellationError { return true }
            catch { return false }
        }
        #expect(await reading.value)
    }

    @Test(.enabled(if: endpoint != nil))
    func aRealMissingTaskPreservesItsHTTPError() async throws {
        let api = NativeAPI(baseURL: try #require(Self.endpoint))
        do { try await api.events("/api/tasks/not-an-existing-task/events") { _ in false }; Issue.record("The missing task unexpectedly streamed.") }
        catch let failure as NativeAPIError { #expect(failure.status == 404) }
    }
}

private actor SSECollection {
    private var events: [NativeSSEEvent] = []
    private var comments = 0
    var hasEvents: Bool { !events.isEmpty }
    func receive(_ record: NativeSSERecord, stopAtComment: Bool = false) -> Bool {
        switch record {
        case .event(let event): if events.count < 500 { events.append(event) }
        case .comment: comments += 1; return !stopAtComment
        }
        return true
    }
    func snapshot() -> ([NativeSSEEvent], Int) { (events, comments) }
}
