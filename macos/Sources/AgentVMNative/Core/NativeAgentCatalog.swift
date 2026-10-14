import Foundation

enum NativeAgentCatalogState: Equatable {
    case loading
    case supported
    case legacy
    case failed(String)
}

extension NativeStore {
    func refreshAgents() async {
        if let task = agentCatalogTask { await task.value; return }
        let task = Task { await fetchAgents() }
        agentCatalogTask = task
        await task.value
    }

    private func fetchAgents() async {
        defer { agentCatalogTask = nil }
        do {
            let result = try await api.request("/api/agents")
            guard case .array = result["agents"], !result["agents"].items.isEmpty,
                  case .array = result["providers"], case .bool = result["claude_token"] else {
                throw NativeAPIError(status: nil, message: "The server returned an incomplete agent catalog.")
            }
            agentCatalog = result
            agentCatalogState = .supported
        } catch let failure as NativeAPIError where failure.status == 404 {
            agentCatalog = .null
            agentCatalogState = .legacy
        } catch is CancellationError {
        } catch {
            agentCatalogState = .failed(error.localizedDescription)
        }
    }
}
