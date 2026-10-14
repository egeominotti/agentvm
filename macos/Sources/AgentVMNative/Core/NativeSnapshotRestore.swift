import Foundation

extension NativeStore {
    func restoreSnapshot(_ id: String) async -> JSON? {
        let path = "/api/snapshots/\(id)/restore"
        guard !busy.contains(path) else { return nil }
        return await perform(path)
    }
}
