import Testing
@testable import AgentVMNative

@Suite("Native launcher validation")
struct LauncherLogicTests {
    @Test("Local branch checks show the branch and shortened commit")
    func branchReference() {
        let check = JSON.object(["branch": .string("main"), "sha": .string("abcdef123")])
        #expect(NativeLauncherLogic.repoReference(check) == "main · abcdef1")
    }

    @Test("Detached local repositories identify HEAD and its commit")
    func detachedReference() {
        let check = JSON.object(["branch": .null, "sha": .string("abcdef123"), "to_clone": .bool(false)])
        #expect(NativeLauncherLogic.repoReference(check) == "detached HEAD · abcdef1")
    }

    @Test("Remote repositories use default branch before their commit is known")
    func remoteReference() {
        let check = JSON.object(["branch": .null, "sha": .null, "to_clone": .bool(true)])
        #expect(NativeLauncherLogic.repoReference(check) == "default branch")
    }

    @Test("Automatic tasks require a prompt after all launch prerequisites pass")
    func promptRequired() {
        let status = JSON.object(["golden": .bool(true), "token": .bool(true)])
        let check = JSON.object(["ok": .bool(true)])
        #expect(NativeLauncherLogic.blocker(status: status, connected: true, repo: "/tmp/repo", check: check,
                                          interactive: false, prompt: "  ") == "Write a task for Claude.")
        #expect(NativeLauncherLogic.blocker(status: status, connected: true, repo: "/tmp/repo", check: check,
                                          interactive: true, prompt: "") == nil)
    }

    @Test("Memory sizes fit the host while preserving the current custom value")
    func memory() {
        #expect(NativeLauncherLogic.memoryChoices(host: 16384, current: 3072) == [1024, 2048, 3072, 4096, 6144, 8192])
        #expect(NativeLauncherLogic.memoryChoices(host: 8192, current: 1024) == [1024])
    }

    @Test("Recent repositories drop blanks and duplicates without rewriting paths")
    func recent() {
        #expect(NativeLauncherLogic.recent(["", " /tmp/repo ", "/tmp/repo", "~/code/another"]) == ["/tmp/repo", "~/code/another"])
    }
}
