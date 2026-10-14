import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native read-only preview policy")
struct NativePreviewPolicyTests {
    @MainActor
    @Test("A closed visibility observer never reports an already queued refresh")
    func closedVisibility() async {
        var events: [Bool] = []
        let view = NativePreviewVisibilityView { events.append($0) }
        view.refresh()
        view.close()
        await withCheckedContinuation { continuation in
            DispatchQueue.main.async { continuation.resume() }
        }
        withExtendedLifetime(view) { #expect(events == [false]) }
    }

    @Test("Preview connections observe Claude without changing the VM terminal size")
    func readOnlyProtocol() throws {
        let request = try #require(NativePreviewPolicy.request(baseURL: URL(string: "http://127.0.0.1:7794")!, id: "machine-1"))
        let url = try #require(request.url)
        let parts = try #require(URLComponents(url: url, resolvingAgainstBaseURL: false))
        #expect(parts.scheme == "ws")
        #expect(parts.path == "/api/tasks/machine-1/pty")
        #expect(parts.queryItems?.contains(URLQueryItem(name: "session", value: "claude")) == true)
        #expect(parts.queryItems?.contains(URLQueryItem(name: "view", value: "true")) == true)
        #expect(parts.queryItems?.contains(URLQueryItem(name: "cols", value: "200")) == true)
        #expect(parts.queryItems?.contains(URLQueryItem(name: "rows", value: "60")) == true)
        #expect(request.value(forHTTPHeaderField: "Origin") == "http://127.0.0.1:7794")
    }

    @Test("Previews cannot connect to remote origins or send credentials in a URL")
    func localOnly() {
        for origin in ["https://example.com", "http://example.com:7794", "http://secret@127.0.0.1:7794", "http://127.0.0.1"] {
            #expect(NativePreviewPolicy.request(baseURL: URL(string: origin)!, id: "machine-1") == nil)
        }
    }

    @Test("Only visible ready interactive machines stream while the app is active")
    func visibility() {
        #expect(NativePreviewPolicy.shouldStream(visible: true, active: true, running: true, interactive: true, ready: true))
        for missing in 0..<5 {
            let values = (0..<5).map { $0 != missing }
            #expect(!NativePreviewPolicy.shouldStream(visible: values[0], active: values[1], running: values[2], interactive: values[3], ready: values[4]))
        }
    }

    @Test("Split UTF-8 output stays in byte order until the parser receives it")
    func binaryOrdering() {
        var buffer = NativePreviewBuffer()
        let first = buffer.append(Data([0xF0, 0x9F]))
        let second = buffer.append(Data([0x98, 0x80, 0x0A]))
        let bytes = buffer.take()
        let empty = buffer.take()
        #expect(first)
        #expect(second)
        #expect(bytes == Data("😀\n".utf8))
        #expect(empty.isEmpty)
    }

    @Test("Burst output stays bounded and a paused preview discards undelivered data")
    func outputBudget() {
        var buffer = NativePreviewBuffer()
        let first = buffer.append(Data(repeating: 97, count: 600_000))
        let overflow = buffer.append(Data(repeating: 98, count: 600_000))
        let retained = buffer.take()
        let whole = buffer.append(Data(repeating: 99, count: 1 << 20))
        #expect(first)
        #expect(!overflow)
        #expect(retained.count == 600_000)
        #expect(whole)
        buffer.clear()
        let cleared = buffer.take()
        let restoredBudget = buffer.append(Data(repeating: 100, count: 1 << 20))
        let oneMore = buffer.append(Data([101]))
        #expect(cleared.isEmpty)
        #expect(restoredBudget)
        #expect(!oneMore)
    }
}
