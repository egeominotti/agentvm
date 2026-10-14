import Testing
@testable import AgentVMNative

@Suite("S3 availability presentation")
struct S3PresentationTests {
    @Test("Repeated curl failures become one useful local service message")
    func localConnectionFailure() {
        let raw = Array(repeating: "curl: (7) Failed to connect to 127.0.0.1 port 9100: Couldn't connect to server", count: 4)
            .joined(separator: "\n")
        #expect(NativeS3Presentation.message(raw: raw, endpoint: "http://127.0.0.1:9100") ==
                "The S3 server at http://127.0.0.1:9100 is not reachable. Start the local S3 service, then retry.")
    }

    @Test("Remote connection errors never suggest starting a local service")
    func remoteConnectionFailure() {
        let message = NativeS3Presentation.message(raw: "curl: (7) Failed to connect to s3.example.com",
                                                  endpoint: "https://s3.example.com")
        #expect(message.contains("https://s3.example.com"))
        #expect(message.contains("not reachable"))
        #expect(!message.contains("Start the local S3"))
    }

    @Test("Credential failures retain the distinction from connection failures")
    func credentialFailures() {
        for reason in ["AccessDenied", "InvalidAccessKeyId"] {
            let message = NativeS3Presentation.message(raw: "S3 answered 403: \(reason)", endpoint: "https://s3.example.com")
            #expect(!message.contains("not reachable"))
            #expect(message.localizedCaseInsensitiveContains(reason) || message.localizedCaseInsensitiveContains("key"))
        }
    }

    @Test("A saved configuration becomes connected only after a successful check")
    func status() {
        #expect(NativeS3Presentation.status(configured: true, checking: true, verified: false, failure: nil) == "Testing…")
        #expect(NativeS3Presentation.status(configured: true, checking: false, verified: true, failure: "offline") == "Unavailable")
        #expect(NativeS3Presentation.status(configured: true, checking: false, verified: true, failure: nil) == "Connected")
        #expect(NativeS3Presentation.status(configured: true, checking: false, verified: false, failure: nil) == "Saved; not checked")
        #expect(NativeS3Presentation.status(configured: false, checking: false, verified: false, failure: nil) == "Not set up")
    }
}
