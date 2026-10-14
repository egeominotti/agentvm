import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native real completed agent output", .serialized)
struct CompletedOutputIntegrationTests {
    private static func endpoint(_ key: String) -> URL? {
        guard let value = ProcessInfo.processInfo.environment[key], let url = URL(string: value),
              url.scheme == "http", url.host == "127.0.0.1", let port = url.port, port != 7777 else { return nil }
        return url
    }
    private static let completedURL = endpoint("AGENTVM_NATIVE_COMPLETED_TEST_URL")
    private static let completedID = ProcessInfo.processInfo.environment["AGENTVM_NATIVE_COMPLETED_TASK_ID"]
    private static let pagedURL = endpoint("AGENTVM_NATIVE_PAGED_OUTPUT_TEST_URL")
    private static let pagedID = ProcessInfo.processInfo.environment["AGENTVM_NATIVE_PAGED_OUTPUT_TASK_ID"]
    private static let legacyURL = endpoint("AGENTVM_NATIVE_SSE_TEST_URL")
    private static let legacyID = ProcessInfo.processInfo.environment["AGENTVM_NATIVE_SSE_TASK_ID"]

    @MainActor
    @Test(.enabled(if: completedURL != nil && completedID != nil), .timeLimit(.minutes(1)))
    func aRealCompletedCustomAgentFinishesWithoutOpeningAnIdleStream() async throws {
        let (folder, process, url) = try await observer(target: #require(Self.completedURL))
        defer { process.terminate(); process.waitUntilExit(); try? FileManager.default.removeItem(at: folder) }
        let feed = NativeAgentOutputFeed(api: NativeAPI(baseURL: url))
        let id = try #require(Self.completedID)
        var finished = false
        let reading = Task { await feed.run(id: id, ended: true); finished = true }
        defer { reading.cancel() }
        for _ in 0..<300 {
            if finished { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        let returnedPromptly = finished
        reading.cancel(); await reading.value
        #expect(returnedPromptly)
        #expect(feed.loaded)
        #expect(feed.state == "no_changes")
        #expect(feed.entries.contains { $0.text == "native-agent-output-ok" })
        #expect(feed.error == nil)
        #expect(!feed.connected)
        let counts = try await NativeAPI(baseURL: url).request("/__native_counts")
        #expect(counts["GET /api/tasks/\(id)/output"].integer == 1)
        #expect(counts["GET /api/tasks/\(id)/events"].integer == 0)
    }

    @MainActor
    @Test(.enabled(if: pagedURL != nil && pagedID != nil), .timeLimit(.minutes(1)))
    func aRealCompletedAgentDrainsAllFivePagesWithoutLosingItsTailOrCountingGapsTwice() async throws {
        let (folder, process, url) = try await observer(target: #require(Self.pagedURL))
        defer { process.terminate(); process.waitUntilExit(); try? FileManager.default.removeItem(at: folder) }
        let feed = NativeAgentOutputFeed(api: NativeAPI(baseURL: url))
        let id = try #require(Self.pagedID)
        var finished = false
        let reading = Task { await feed.run(id: id, ended: true); finished = true }
        defer { reading.cancel() }
        for _ in 0..<300 {
            if finished { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        let returnedPromptly = finished
        reading.cancel(); await reading.value
        #expect(returnedPromptly)
        #expect(feed.loaded && !feed.connected && feed.error == nil)
        #expect(feed.entries.count == 498)
        #expect(feed.entries.first?.id == "108")
        #expect(feed.entries.last?.id == "605")
        let ordered = feed.entries.map(\.text) == (103...600).map { String(format: "native-page-%04d", $0) }
        #expect(ordered)
        #expect(feed.state == "no_changes")
        #expect(feed.dropped == 107)
        let bytes = feed.entries.reduce(0) { $0 + $1.id.utf8.count + $1.title.utf8.count + $1.text.utf8.count }
        #expect(bytes <= 1 << 20)
        let counts = try await NativeAPI(baseURL: url).request("/__native_counts")
        #expect(counts["GET /api/tasks/\(id)/output"].integer == 5)
        #expect(counts["GET /api/tasks/\(id)/events"].integer == 0)
    }

    @MainActor
    @Test(.enabled(if: legacyURL != nil && legacyID != nil), .timeLimit(.minutes(1)))
    func legacyFiniteEndpointFallsBackToRealSSEAndCancelsCleanly() async throws {
        let (folder, process, url) = try await observer(target: #require(Self.legacyURL))
        defer { process.terminate(); process.waitUntilExit(); try? FileManager.default.removeItem(at: folder) }
        let feed = NativeAgentOutputFeed(api: NativeAPI(baseURL: url))
        let id = try #require(Self.legacyID)
        let reading = Task { await feed.run(id: id, ended: true) }
        defer { reading.cancel() }
        for _ in 0..<300 {
            if feed.loaded { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        let loaded = feed.loaded
        let beforeCancellation = ContinuousClock.now
        reading.cancel(); await reading.value
        #expect(loaded)
        #expect(beforeCancellation.duration(to: .now) < .seconds(2))
        #expect(feed.error == nil)
        #expect(!feed.connected)
        let counts = try await NativeAPI(baseURL: url).request("/__native_counts")
        #expect(counts["GET /api/tasks/\(id)/output"].integer == 1)
        #expect(counts["GET /api/tasks/\(id)/events"].integer == 1)
    }

    private func observer(target: URL) async throws -> (URL, Process, URL) {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        do {
            let (process, url) = try await RealHTTPObserver.start(target: target, folder: folder)
            return (folder, process, url)
        } catch { try? FileManager.default.removeItem(at: folder); throw error }
    }
}
