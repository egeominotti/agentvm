import Foundation
import Testing
@testable import AgentVMNative

@Suite("AgentSettingsPolicyTests")
struct AgentSettingsPolicyTests {
    @Test
    func testProviderConfigurationSaveCannotForwardStoredCredentialFields() {
        let input = JSON.object(["endpoint": .string("http://127.0.0.1:11434/v1"), "protocol": .string("openai"),
                                 "requires_key": .bool(false), "token": .string("do-not-forward"), "has_key": .bool(true)])
        let body = NativeAgentSettingsPolicy.providerUpdate(config: input)
        #expect(body["config"].fields.count == 3)
        #expect(body["config"]["endpoint"].text == "http://127.0.0.1:11434/v1")
        #expect(!body["config"]["requires_key"].flag)
        #expect(body["token"].isNull)
        #expect(body["config"]["token"].isNull)
        #expect(NativeAgentSettingsPolicy.providerUpdate(config: input, token: "  \n")["token"].isNull)
        #expect(NativeAgentSettingsPolicy.providerUpdate(config: input, token: " new-key ")["token"].text == "new-key")
    }

    @Test
    func testProviderEndpointRequiresAnAbsoluteCredentialFreeHTTPURL() {
        for endpoint in ["https://api.openai.com/v1", "HTTPS://api.openai.com/v1", "http://127.0.0.1:11434/v1", "http://[::1]:11434/v1"] {
            #expect(NativeAgentSettingsPolicy.providerBlocker(endpoint: endpoint, protocolID: "openai") == nil)
        }
        for endpoint in ["", "/api", "file:///tmp/provider", "https://user:key@host/v1", "https://host/v1?key=value", "https://host/v1#key"] {
            #expect(NativeAgentSettingsPolicy.providerBlocker(endpoint: endpoint, protocolID: "openai") != nil)
        }
        #expect(NativeAgentSettingsPolicy.providerBlocker(endpoint: "https://host", protocolID: "unknown") != nil)
    }

    @Test
    func testCustomDraftPreservesLiteralCommandsAndDropsBlankOptionalTemplates() {
        let value = NativeAgentSettingsPolicy.customValue(name: " My CLI ", install: "printf '%s\\n' installed\n# installation shell",
                                                          interactive: "my-tool --prompt '{prompt}' --model \"{model}\"",
                                                          headless: " \n", resume: "")
        #expect(value["name"].text == "My CLI")
        #expect(value["install"].text == "printf '%s\\n' installed\n# installation shell")
        #expect(value["interactive"].text == "my-tool --prompt '{prompt}' --model \"{model}\"")
        #expect(value["headless"].isNull)
        #expect(value["resume"].isNull)
        #expect(NativeAgentPolicy.customBlocker(value, interactive: true) == nil)
        #expect(NativeAgentPolicy.customBlocker(value, interactive: false) != nil)
    }

    @Test
    func testDefaultAgentUpdateNormalizesOptionalValuesWithoutChangingWorkspaceSettings() {
        let settings = JSON.object(["agent": .object(["kind": .string("claude")]), "model": .string("sonnet"),
                                    "memory_mb": JSON(3072), "default_repo": .string("/tmp/owned-repo")])
        let selection = JSON.object(["kind": .string("codex"), "provider": .string("openai"),
                                     "version": .string("stable")])
        let saved = NativeAgentSettingsPolicy.defaultsUpdate(settings: settings, selection: selection, version: "")
        #expect(saved["agent"]["kind"].text == "codex")
        #expect(saved["agent"]["provider"].text == "openai")
        #expect(saved["agent"]["version"].isNull)
        #expect(saved["model"].text == "default")
        #expect(saved["memory_mb"].integer == 3072)
        #expect(saved["default_repo"].text == "/tmp/owned-repo")
        #expect(settings["model"].text == "sonnet")
    }

    @Test
    func testReusableCustomCommandsReplaceAnEmbeddedDefaultWithoutLosingOldInlineOnlySettings() {
        let inline = JSON.object(["name": .string("Old CLI"), "interactive": .string("old-cli")])
        let current = JSON.object(["name": .string("New CLI"), "interactive": .string("new-cli")])
        let selection = JSON.object(["kind": .string("custom"), "custom": inline])
        let settings = JSON.object(["agent": selection, "custom_cli": current])
        #expect(NativeAgentSettingsPolicy.reusableCustom(settings: settings)["name"].text == "New CLI")
        let saved = NativeAgentSettingsPolicy.defaultsUpdate(settings: settings, selection: selection, version: "latest")
        #expect(saved["agent"]["custom"].isNull)
        #expect(saved["agent"]["version"].text == "latest")
        #expect(saved["custom_cli"]["interactive"].text == "new-cli")

        let inlineOnly = JSON.object(["agent": selection])
        #expect(NativeAgentSettingsPolicy.reusableCustom(settings: inlineOnly)["name"].text == "Old CLI")
        let migrated = NativeAgentSettingsPolicy.defaultsUpdate(settings: inlineOnly, selection: selection, version: "")
        #expect(migrated["agent"]["custom"].isNull)
        #expect(migrated["custom_cli"]["interactive"].text == "old-cli")
    }

    @Test
    func testSwitchingCodingCLIsUsesRuntimeModelsAndKeepsSameKindAPIOverrides() {
        let previous = JSON.object(["agent": .object(["kind": .string("claude")]), "model": .string("sonnet"),
                                    "memory_mb": JSON(3072)])
        let codex = JSON.object(["kind": .string("codex"), "provider": .string("local")])
        let switched = NativeAgentSettingsPolicy.defaultsUpdate(settings: previous, selection: codex,
                                                                version: "latest")
        #expect(switched["model"].text == "default")
        #expect(switched["memory_mb"].integer == 3072)

        let explicit = JSON.object(["agent": codex, "model": .string("provider/model")])
        let edited = NativeAgentSettingsPolicy.defaultsUpdate(settings: explicit, selection: codex,
                                                             version: "1.2.3")
        #expect(edited["model"].text == "provider/model")
        #expect(edited["agent"]["version"].text == "1.2.3")
        #expect(previous["model"].text == "sonnet")
    }
}
