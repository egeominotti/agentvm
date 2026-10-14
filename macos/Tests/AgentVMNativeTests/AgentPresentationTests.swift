import Testing
@testable import AgentVMNative

@Suite("Native agent presentation")
struct AgentPresentationTests {
    @Test
    func oldTasksKeepTheirClaudeTerminalAndConversationAnalytics() {
        let task = JSON.object(["claude_version": .string("2.1.299")])
        #expect(NativeAgentPresentation.name(task) == "Claude Code")
        #expect(NativeAgentPresentation.terminalLabel("claude", task: task) == "Claude")
        #expect(NativeAgentPresentation.showsClaudeAnalytics(task))
        #expect(NativeAgentPresentation.version(task) == "2.1.299")
    }

    @Test
    func newAgentsHaveTheirOwnVisibleNameWhileKeepingTheCompatibleWireSession() {
        for kind in ["codex", "gemini", "opencode", "qwen", "copilot", "kilo", "aider", "goose", "custom"] {
            let task = JSON.object(["agent": .object(["kind": .string(kind), "version": .string("latest")]),
                                    "claude_version": .string("2.1.299")])
            #expect(NativeAgentPresentation.terminalLabel("claude", task: task) == NativeAgentPresentation.name(task))
            #expect(!NativeAgentPresentation.showsClaudeAnalytics(task))
            #expect(NativeAgentPresentation.version(task) == "latest")
            #expect(NativeAgentPresentation.terminalLabel("shell", task: task) == "Shell")
            #expect(NativeAgentPresentation.terminalLabel("shell-3", task: task) == "Shell 3")
        }
    }

    @Test
    func aNonClaudeSnapshotShowsItsOwnCLIDefaultModel() {
        let codex = JSON.object(["agent": .object(["kind": .string("codex")]), "model": .string("default")])
        #expect(NativeAgentPresentation.model(codex) == "CLI default")
        let claude = JSON.object(["model": .string("opus")])
        #expect(NativeAgentPresentation.model(claude) == "Opus")
    }
}
