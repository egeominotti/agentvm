import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native launch capacity")
struct LauncherCapacityTests {
    @Test("Capacity reserves macOS memory and accounts for already committed VM RAM")
    func availableCapacity() {
        let capacity = NativeLauncherLogic.capacity(status: status(ram: 65536, committed: 4096, running: 1), memory: 4096)
        #expect(capacity?.freeMB == 53248)
        #expect(capacity?.fit == 13)
        #expect(capacity?.starts == 13)
        #expect(capacity?.waitingFor == nil)
    }

    @Test("A full concurrency limit queues the launch even when memory is available")
    func concurrencyQueue() {
        let capacity = NativeLauncherLogic.capacity(status: status(ram: 65536, committed: 4096, running: 14), memory: 4096)
        #expect(capacity?.freeMB == 53248)
        #expect(capacity?.fit == 13)
        #expect(capacity?.starts == 0)
        #expect(capacity?.waitingFor == "concurrency")
    }

    @Test("A VM larger than the remaining RAM queues for memory")
    func memoryQueue() {
        let capacity = NativeLauncherLogic.capacity(status: status(ram: 16384, committed: 4096, running: 1), memory: 6144)
        #expect(capacity?.freeMB == 4096)
        #expect(capacity?.fit == 0)
        #expect(capacity?.starts == 0)
        #expect(capacity?.waitingFor == "memory")
    }

    @Test("Overcommitted RAM and slots never produce negative capacity")
    func overcommittedCapacity() {
        let capacity = NativeLauncherLogic.capacity(status: status(ram: 8192, committed: 16384, running: 20), memory: 1024)
        #expect(capacity?.freeMB == 0)
        #expect(capacity?.fit == 0)
        #expect(capacity?.starts == 0)
        #expect(capacity?.waitingFor == "memory")
    }

    @Test("Unknown status and invalid memory do not invent a capacity estimate")
    func invalidInputs() {
        #expect(NativeLauncherLogic.capacity(status: .null, memory: 4096) == nil)
        #expect(NativeLauncherLogic.capacity(status: .object([:]), memory: 4096) == nil)
        let known = status(ram: 65536, committed: 4096, running: 1)
        #expect(NativeLauncherLogic.capacity(status: known, memory: 0) == nil)
        #expect(NativeLauncherLogic.capacity(status: known, memory: -1) == nil)
    }

    @Test(.enabled(if: NativeTestEnvironment.endpoint != nil))
    func realEngineStatusProducesAValidCapacityEstimate() async throws {
        let api = NativeAPI(baseURL: try #require(NativeTestEnvironment.endpoint))
        let status = try await api.request("/api/status")
        let capacity = try #require(NativeLauncherLogic.capacity(status: status, memory: 4096))
        #expect(capacity.freeMB >= 0)
        #expect(capacity.fit >= 0)
        #expect(capacity.starts >= 0)
        #expect(capacity.starts <= max(0, status["concurrency"].integer - status["running"].integer))
    }

    private func status(ram: Int, committed: Int, running: Int) -> JSON {
        .object(["host": .object(["ram_mb": JSON(ram)]), "ram_committed_mb": JSON(committed),
                 "running": JSON(running), "concurrency": JSON(14)])
    }
}
