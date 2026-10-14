import Foundation

struct RuntimeConfiguration {
    let home: URL
    let endpoint: URL

    init(environment: [String: String], userHome: URL = FileManager.default.homeDirectoryForCurrentUser) throws {
        let portText = environment["AGENTVM_PORT"] ?? "7777"
        guard let port = Int(portText), (1...65535).contains(port) else {
            throw RuntimeError.message("AGENTVM_PORT must be between 1 and 65535.")
        }
        home = environment["AGENTVM_HOME"].map { URL(fileURLWithPath: $0, isDirectory: true) }
            ?? userHome.appendingPathComponent("AgentVMs", isDirectory: true)
        endpoint = URL(string: "http://127.0.0.1:\(port)")!
    }

    static func isAgentVM(_ value: JSON) -> Bool {
        guard case .bool = value["golden"], case .bool = value["token"],
              case .number = value["concurrency"], case .number = value["running"],
              case .object = value["host"] else { return false }
        return value["host"]["cpus"].integer > 0
    }
}

enum RuntimeError: LocalizedError {
    case message(String)
    var errorDescription: String? {
        switch self { case .message(let message): return message }
    }
}
