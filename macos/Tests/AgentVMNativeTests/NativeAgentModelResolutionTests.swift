import Testing
@testable import AgentVMNative

@Suite("Native launcher automatic model resolution")
struct NativeAgentModelResolutionTests {
    @Test("Custom model placeholders need a provider for automatic resolution, including resume commands")
    func customResolutionProvider() {
        let catalog = JSON.object([
            "claude_token": .bool(false),
            "agents": .array([.object(["kind": .string("custom"), "name": .string("Custom CLI"), "providers": .array([.string("local")]),
                                       "headless": .bool(true), "resume": .bool(true), "requires_model": .bool(false)])]),
            "providers": .array([.object(["id": .string("local"), "name": .string("Local"), "has_key": .bool(false),
                                          "config": .object(["endpoint": .string("http://127.0.0.1:11434/v1"),
                                                             "protocol": .string("openai"), "requires_key": .bool(false)])])])
        ])
        for key in ["interactive", "headless", "resume"] {
            var custom = JSON.object(["name": .string("Tool"), "interactive": .string("tool"), "headless": .string("tool run --prompt {prompt}")])
            custom[key] = .string("tool --model {model}")
            var selection = JSON.object(["kind": .string("custom"), "custom": custom])
            #expect(NativeAgentPolicy.configurationBlocker(selection: selection, catalog: catalog) != nil)
            #expect(NativeAgentPolicy.readiness(selection: selection, catalog: catalog, legacyToken: false, interactive: true) != nil)
            selection["provider"] = .string("local")
            #expect(NativeAgentPolicy.configurationBlocker(selection: selection, catalog: catalog) == nil)
            #expect(NativeAgentPolicy.readiness(selection: selection, catalog: catalog, legacyToken: false, interactive: true) == nil)
            #expect(NativeAgentPolicy.readiness(selection: selection, catalog: catalog, legacyToken: false, interactive: false) == nil)
        }
    }

    @Test("Every UI launch omits saved models while retaining its CLI and provider")
    func omitSavedModels() {
        let kinds = ["claude", "codex", "gemini", "opencode", "qwen", "copilot", "kilo", "aider", "goose", "custom"]
        let custom = JSON.object(["name": .string("Tool"), "interactive": .string("tool --model {model}")])
        for kind in kinds {
            let selection = JSON.object(["kind": .string(kind), "provider": .string("local"), "custom": custom])
            for saved in ["opus", "vendor/wrong-provider-model", "invalid model; value"] {
                let base = JSON.object(["repo_path": .string("/tmp/repo"), "interactive": .bool(true), "model": .string(saved)])
                let modern = NativeAgentPolicy.launchBody(base: base, selection: selection, version: "1.2.3", supported: true)
                #expect(modern.fields["model"] == nil)
                #expect(modern["repo_path"].text == "/tmp/repo")
                #expect(modern["agent"]["kind"].text == kind)
                #expect(modern["agent"]["provider"].text == "local")
                #expect(modern["agent"]["version"].text == "1.2.3")
                let legacy = NativeAgentPolicy.launchBody(base: base, selection: selection, version: "2.1.294", supported: false)
                #expect(legacy.fields["model"] == nil)
                #expect(legacy.fields["agent"] == nil)
                #expect(legacy["claude_version"].text == "2.1.294")
            }
        }
    }
}
