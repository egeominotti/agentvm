import Foundation

extension NativeStore {
    func performMachineDeletion(ids: [String]) async -> [String] {
        guard !ids.isEmpty else { return [] }
        error = nil
        notice = nil
        var completed: [String] = []
        for id in ids {
            do {
                _ = try await api.request("/api/tasks/\(id)", method: "DELETE")
                completed.append(id)
            } catch is CancellationError { break }
            catch { self.error = error.localizedDescription; break }
        }
        await refresh()
        return completed
    }
}
