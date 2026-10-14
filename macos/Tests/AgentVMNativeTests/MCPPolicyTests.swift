import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native MCP client configuration")
struct MCPPolicyTests {
    private let server = URL(fileURLWithPath: "/Applications/agentvm Native.app/Contents/Resources/runtime/bin/agentvm-mcp")

    @Test
    func configurationUsesTheBundledExecutableAndThisWorkspaceEndpoint() throws {
        let endpoint = URL(string: "http://127.0.0.1:7794")!
        let result = try #require(NativeMCPPolicy.configuration(server: server, endpoint: endpoint))
        let entry = result["mcpServers"]["agentvm"]
        #expect(entry["command"].text == server.path)
        #expect(entry["args"].items.isEmpty)
        #expect(entry["env"] == .object(["AGENTVM_URL": .string(endpoint.absoluteString)]))
        #expect(try JSONDecoder().decode(JSON.self, from: Data(result.pretty.utf8)) == result)
    }

    @Test
    func clientConfigurationNeverCopiesSecretsOrAnUnsupportedConnection() {
        for raw in ["https://127.0.0.1:7794", "http://remote.example:7794", "http://user:secret@127.0.0.1:7794",
                    "http://127.0.0.1:7794?token=secret", "http://127.0.0.1:7794/#secret", "http://127.0.0.1", "http://127.0.0.1:7794/api"] {
            #expect(NativeMCPPolicy.configuration(server: server, endpoint: URL(string: raw)!) == nil)
        }
        #expect(NativeMCPPolicy.configuration(server: URL(string: "https://example.com/server")!,
                                              endpoint: URL(string: "http://127.0.0.1:7794")!) == nil)
    }
}
