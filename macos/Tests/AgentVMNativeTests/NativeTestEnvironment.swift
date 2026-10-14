import Foundation

enum NativeTestEnvironment {
    static var endpoint: URL? {
        guard let value = ProcessInfo.processInfo.environment["AGENTVM_NATIVE_TEST_URL"],
              let url = URL(string: value), url.scheme == "http",
              ["127.0.0.1", "localhost"].contains(url.host ?? ""), url.port != nil else { return nil }
        return url
    }

    static var canChangeSettings: Bool { endpoint != nil && endpoint?.port != 7777 }
    static var snapshotEndpoint: URL? {
        guard let value = ProcessInfo.processInfo.environment["AGENTVM_NATIVE_SNAPSHOT_TEST_URL"] else { return endpoint }
        guard let url = URL(string: value), url.scheme == "http",
              ["127.0.0.1", "localhost"].contains(url.host ?? ""), url.port != nil else { return nil }
        return url
    }
    static var canChangeSnapshots: Bool { snapshotEndpoint != nil && snapshotEndpoint?.port != 7777 }
    static var outageEndpoint: URL? {
        guard let value = ProcessInfo.processInfo.environment["AGENTVM_NATIVE_ERROR_TEST_URL"],
              let url = URL(string: value), url.scheme == "http",
              ["127.0.0.1", "localhost"].contains(url.host ?? ""), url.port != nil else { return nil }
        return url
    }
    static var s3RecoveryEndpoint: URL? {
        guard let value = ProcessInfo.processInfo.environment["AGENTVM_NATIVE_S3_RECOVERY_TEST_URL"],
              let url = URL(string: value), url.scheme == "http",
              ["127.0.0.1", "localhost"].contains(url.host ?? ""),
              let port = url.port, port != 7777 else { return nil }
        return url
    }
}
