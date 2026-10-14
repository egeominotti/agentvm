import Foundation

extension NativeStore {
    /// Keep archive mutations sequential so a selection does not compete for this Mac's disk.
    func performSnapshotBatch(ids: [String], backup: Bool) async -> [String] {
        guard !ids.isEmpty else { return [] }
        error = nil
        notice = nil
        var completed: [String] = []
        for id in ids {
            let path = "/api/snapshots/\(id)" + (backup ? "/backup" : "")
            do {
                _ = try await api.request(path, method: backup ? "POST" : "DELETE")
                completed.append(id)
            } catch is CancellationError { break }
            catch {
                self.error = backup ? NativeS3Presentation.message(raw: error.localizedDescription,
                                                                  endpoint: settings["s3"]["endpoint"].text)
                    : error.localizedDescription
                break
            }
        }
        if completed.count == ids.count {
            notice = "\(completed.count) snapshots \(backup ? "backed up to S3" : "deleted")"
        } else if !completed.isEmpty {
            notice = "\(completed.count) of \(ids.count) snapshots \(backup ? "backed up" : "deleted")"
        }
        // One refresh also reconciles a partial batch; local deletion needs no S3 listing.
        await refreshSnapshots(includeBackups: backup)
        return completed
    }
}
