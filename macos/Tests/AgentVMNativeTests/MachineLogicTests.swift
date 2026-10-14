import Testing
@testable import AgentVMNative

@Suite("Native machine decisions")
struct MachineLogicTests {
    @Test("Save and Close exclude each other only for the same machine")
    func saveOrCloseBusy() {
        let base = "/api/tasks/machine-1"
        #expect(NativeMachineLogic.saveOrCloseBusy(base: base, busy: [base + "/save"]))
        #expect(NativeMachineLogic.saveOrCloseBusy(base: base, busy: [base + "/close"]))
        #expect(!NativeMachineLogic.saveOrCloseBusy(base: base, busy: ["/api/tasks/another/save"]))
        #expect(!NativeMachineLogic.saveOrCloseBusy(base: base, busy: []))
    }

    @Test("Reopening restores shell tabs and mounts only the selected terminal")
    func restoredSessions() {
        let restored = NativeMachineLogic.restoreSessions(stored: "shell-3", extras: ["shell-9", "shell-2", "shell-3"])
        #expect(restored.active == "shell-3")
        #expect(restored.extras == ["shell-2", "shell-3", "shell-9"])
        #expect(restored.mounted == Set(["claude", "shell-3"]))
        let shell = NativeMachineLogic.restoreSessions(stored: "shell", extras: ["shell-2"])
        #expect(shell.active == "shell")
        #expect(shell.mounted == Set(["claude", "shell"]))
        for stored in ["invalid", "shell-4"] {
            let closed = NativeMachineLogic.restoreSessions(stored: stored, extras: ["shell-2", "shell-3"])
            #expect(closed.active == "claude")
            #expect(closed.mounted == Set(["claude"]))
        }
    }

    @Test("Readiness takes precedence over interactive activity")
    func readiness() {
        let task = JSON.object([
            "status": .object(["state": .string("running")]),
            "interactive": .bool(true), "ready": .bool(false), "activity": .string("waiting")
        ])
        #expect(NativeMachineLogic.category(task) == "starting")
        #expect(NativeMachineLogic.status(task) == "Starting")
    }

    @Test("Search includes branch and full machine id")
    func search() {
        let task = JSON.object([
            "id": .string("agent-abcdef"), "repo": .string("/Users/test/code/widget"),
            "branch": .string("agent/fix-settings"), "prompt": .string(""), "label": .null
        ])
        #expect(NativeMachineLogic.matches(task, query: "FIX-SETTINGS"))
        #expect(NativeMachineLogic.matches(task, query: "abcdef"))
        #expect(!NativeMachineLogic.matches(task, query: "unrelated"))
    }

    @Test("Extra shell names are valid, unique and limited to eight")
    func shells() {
        #expect(NativeMachineLogic.shells(["shell-9", "bad", "shell-2", "shell-2"]) == ["shell-2", "shell-9"])
        #expect(NativeMachineLogic.nextShell(["shell-2", "shell-4"]) == "shell-3")
        #expect(NativeMachineLogic.nextShell((2...9).map { "shell-\($0)" }) == nil)
    }

    @Test("Diff content that looks like metadata remains visible")
    func diff() {
        let patch = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n--- old\n+++ new"
        let files = NativeMachineLogic.diff(patch)
        #expect(files.count == 1)
        #expect(files.first?.path == "file.txt")
        #expect(files.first?.added == 1)
        #expect(files.first?.removed == 1)
        #expect(files.first?.lines.suffix(2) == ["--- old", "+++ new"])
    }

    @Test("The active session survives reopening while closed extra shells do not reopen")
    func activeSession() {
        #expect(NativeMachineLogic.activeSession("shell", extras: []) == "shell")
        #expect(NativeMachineLogic.activeSession("shell-3", extras: ["shell-3"]) == "shell-3")
        #expect(NativeMachineLogic.activeSession("shell-3", extras: []) == "claude")
        #expect(NativeMachineLogic.activeSession("invalid", extras: ["shell-2"]) == "claude")
        #expect(NativeMachineLogic.activeSession(nil, extras: []) == "claude")
    }

    @Test("Shell paths with quotes and substitutions remain literal")
    func quoting() {
        #expect(NativeMachineLogic.shellQuote("/mnt/job/uploads/photo.png") == "/mnt/job/uploads/photo.png")
        #expect(NativeMachineLogic.shellQuote("/tmp/it's $(secret).txt") == "'/tmp/it'\\''s $(secret).txt'")
    }
}
