import Foundation

extension NativeStore {
    func testS3Connection() async -> Bool {
        guard await perform("/api/settings/s3/test", success: "The S3 bucket accepts uploads") != nil else { return false }
        await refreshSnapshots()
        return true
    }
}
