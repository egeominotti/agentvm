import Foundation
import Testing
@testable import AgentVMNative

@Suite("RuntimeTests")
struct RuntimeTests {
    @Test
    func testExplicitHomeAndPortRemainIsolatedFromTheExistingApp() throws {
        let config = try RuntimeConfiguration(environment: ["AGENTVM_HOME": "/tmp/native-test", "AGENTVM_PORT": "7793"],
                                              userHome: URL(fileURLWithPath: "/Users/test"))
        #expect(config.home.path == "/tmp/native-test")
        #expect(config.endpoint.absoluteString == "http://127.0.0.1:7793")
    }

    @Test
    func testDefaultsShareTheExistingEngineWithoutMovingItsData() throws {
        let config = try RuntimeConfiguration(environment: [:], userHome: URL(fileURLWithPath: "/Users/test"))
        #expect(config.home.path == "/Users/test/AgentVMs")
        #expect(config.endpoint.absoluteString == "http://127.0.0.1:7777")
    }

    @Test
    func testInvalidPortsCannotAccidentallyConnectToTheRealServer() {
        for port in ["0", "-1", "65536", "other"] {
            #expect(throws: (any Error).self) { try RuntimeConfiguration(environment: ["AGENTVM_PORT": port],
                                                          userHome: URL(fileURLWithPath: "/Users/test")) }
        }
    }

    @Test
    func testBackendIdentityRequiresTheActualStatusShape() throws {
        let status = try JSONDecoder().decode(JSON.self, from: Data(#"{"golden":true,"token":false,"host":{"cpus":8,"ram_mb":16384},"concurrency":2,"running":0}"#.utf8))
        #expect(RuntimeConfiguration.isAgentVM(status))
        #expect(!(RuntimeConfiguration.isAgentVM(.object(["healthy": .bool(true)]))))
    }
}
