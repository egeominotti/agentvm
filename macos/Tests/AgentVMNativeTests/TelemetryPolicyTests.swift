import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native bounded telemetry")
struct TelemetryPolicyTests {
    @Test("A short measured segment between gaps remains visible after downsampling")
    func singletonSegment() {
        let points = (0..<1000).map { index in
            JSON.object(["at": JSON(index), "disk_read_bps": [5, 7].contains(index) ? .null : JSON(index)])
        }
        let sampled = NativeTelemetryPolicy.sample(points: points, key: "disk_read_bps", limit: 300)
        #expect(sampled.count <= 300)
        for index in [0, 4, 6, 8, 999] { #expect(sampled.contains { $0.index == index }) }
        #expect(Set(sampled.map(\.segment)).count == 3)
    }

    @Test("Long series retain endpoints and narrow extrema within a bounded mark count")
    func boundedExtrema() {
        let points: [JSON] = (0..<1024).map { index -> JSON in
            let value: Int
            if index == 523 { value = 999 }
            else if index == 601 { value = 1 }
            else { value = 100 + index % 20 }
            return JSON.object(["at": JSON(index), "cpu_pct": JSON(value)])
        }
        let sampled = NativeTelemetryPolicy.sample(points: points, key: "cpu_pct", limit: 32)
        #expect(sampled.count <= 32)
        #expect(sampled.first?.index == 0)
        #expect(sampled.last?.index == 1023)
        #expect(sampled.contains { $0.index == 523 && $0.value == 999 })
        #expect(sampled.contains { $0.index == 601 && $0.value == 1 })
        let indices: [Int] = sampled.map { $0.index }
        #expect(indices == indices.sorted())
        #expect(Set(indices).count == indices.count)
    }

    @Test("Downsampling preserves separate segments across dropped missing readings")
    func missingReadings() throws {
        let points = (0..<1000).map { index in
            JSON.object(["at": JSON(index), "disk_read_bps": (400..<450).contains(index) ? .null : JSON(index)])
        }
        let sampled = NativeTelemetryPolicy.sample(points: points, key: "disk_read_bps", limit: 32)
        #expect(sampled.count <= 32)
        #expect(!sampled.contains { (400..<450).contains($0.index) })
        let before = try #require(sampled.last { $0.index < 400 })
        let after = try #require(sampled.first { $0.index >= 450 })
        #expect(before.segment != after.segment)
        #expect(sampled.first?.index == 0)
        #expect(sampled.last?.index == 999)
    }

    @Test("Small and entirely missing series retain exact measured values")
    func smallSeries() {
        let points: [JSON] = [.object(["at": JSON(1), "cpu_pct": JSON(9)]),
                              .object(["at": JSON(2), "cpu_pct": .null]),
                              .object(["at": JSON(3), "cpu_pct": JSON(7)])]
        let sampled = NativeTelemetryPolicy.sample(points: points, key: "cpu_pct", limit: 32)
        #expect(sampled.map(\.value) == [9, 7])
        #expect(sampled.map(\.at) == [1, 3])
        #expect(sampled.first?.segment != sampled.last?.segment)
        #expect(NativeTelemetryPolicy.sample(points: points, key: "unknown").isEmpty)
        #expect(NativeTelemetryPolicy.sample(points: [], key: "cpu_pct").isEmpty)
    }

    @Test(.enabled(if: NativeTestEnvironment.snapshotEndpoint != nil && ProcessInfo.processInfo.environment["AGENTVM_NATIVE_TELEMETRY_TASK_ID"] != nil))
    func realVMHistoryHasBoundedSamplesWithoutLosingPeaks() async throws {
        let api = NativeAPI(baseURL: try #require(NativeTestEnvironment.snapshotEndpoint))
        let id = try #require(ProcessInfo.processInfo.environment["AGENTVM_NATIVE_TELEMETRY_TASK_ID"])
        let points = try await api.request("/api/tasks/\(id)/telemetry?range=all")["points"].items
        try #require(points.count > 32)
        for key in ["cpu_pct", "mem_used_mb", "disk_read_bps", "net_rx_bps"] {
            let measured = points.enumerated().filter { !$0.element[key].isNull }
            let sampled = NativeTelemetryPolicy.sample(points: points, key: key, limit: 32)
            #expect(sampled.count <= 32)
            #expect(sampled.first?.index == measured.first?.offset)
            #expect(sampled.last?.index == measured.last?.offset)
            #expect(sampled.map(\.value).max() == measured.map { $0.element[key].number }.max())
            #expect(sampled.map(\.value).min() == measured.map { $0.element[key].number }.min())
        }
    }
}
