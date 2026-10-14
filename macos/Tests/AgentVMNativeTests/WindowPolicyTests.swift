import Testing
@testable import AgentVMNative

@Suite("Native workspace window reuse")
struct WindowPolicyTests {
    @Test("Open and Settings reuse an existing workspace beside unrelated windows")
    func reuseWorkspace() {
        #expect(NativeWindowPolicy.existingIndex(title: "agentvm Native",
                titles: ["Choose a file", "agentvm Native", "Open logs"]) == 1)
    }

    @Test("Verification windows reuse only their own isolated profile")
    func reuseVerificationWorkspace() {
        #expect(NativeWindowPolicy.existingIndex(title: "agentvm Verification :7794",
                titles: ["agentvm Native", "agentvm Verification :7793", "agentvm Verification :7794"]) == 2)
    }

    @Test("A closed workspace is created again and existing duplicates choose the first")
    func createAfterClosing() {
        #expect(NativeWindowPolicy.existingIndex(title: "agentvm Native", titles: []) == nil)
        #expect(NativeWindowPolicy.existingIndex(title: "agentvm Native", titles: ["Choose a file"]) == nil)
        #expect(NativeWindowPolicy.existingIndex(title: "agentvm Native",
                titles: ["agentvm Native", "agentvm Native"]) == 0)
    }
}
