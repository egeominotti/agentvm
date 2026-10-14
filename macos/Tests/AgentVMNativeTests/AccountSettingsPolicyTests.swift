import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native account defaults")
struct AccountSettingsPolicyTests {
    @Test
    func accountDefaultsKeepTheirIdentityWithoutAnAPIProvider() {
        let settings = JSON.object(["agent": .object(["kind": .string("codex")]),
                                    "model": .string("explicit/api-model"), "memory_mb": JSON(3072)])
        let draft = JSON.object(["kind": .string("codex"), "auth": .string("account"),
                                 "account_kind": .string("chatgpt"), "provider": .string("openai")])
        let saved = NativeAgentSettingsPolicy.defaultsUpdate(settings: settings, selection: draft, version: "1.2.3")
        #expect(saved["agent"]["auth"].text == "account")
        #expect(saved["agent"]["account_kind"].text == "chatgpt")
        #expect(saved["agent"]["provider"].isNull)
        #expect(saved["agent"]["version"].text == "1.2.3")
        #expect(saved["model"].text == "explicit/api-model")
        #expect(saved["memory_mb"].integer == 3072)
    }

    @Test
    func explicitAPIAuthenticationCannotRetainAnOldAccountKind() {
        let draft = JSON.object(["kind": .string("claude"), "auth": .string("api"),
                                 "account_kind": .string("claude"), "provider": .string("anthropic")])
        let saved = NativeAgentSettingsPolicy.defaultsUpdate(settings: .object([:]), selection: draft, version: "")
        #expect(saved["agent"]["auth"].text == "api")
        #expect(saved["agent"]["account_kind"].isNull)
        #expect(saved["agent"]["provider"].text == "anthropic")
        #expect(saved["agent"]["version"].isNull)
    }

    @Test
    func anImplicitAccountSelectionStillUsesAccountAuthentication() {
        let draft = JSON.object(["kind": .string("goose"), "account_kind": .string("claude")])
        let saved = NativeAgentSettingsPolicy.defaultsUpdate(settings: .object([:]), selection: draft, version: "latest")
        #expect(saved["agent"]["auth"].text == "account")
        #expect(saved["agent"]["account_kind"].text == "claude")
        #expect(saved["agent"]["provider"].isNull)
    }

    @Test
    func oldClaudeDefaultsRemainCompatibleWithEnginesWithoutAccountRoutes() {
        let draft = JSON.object(["kind": .string("claude")])
        let saved = NativeAgentSettingsPolicy.defaultsUpdate(settings: .object([:]), selection: draft, version: "stable")
        #expect(saved["agent"]["auth"].isNull)
        #expect(saved["agent"]["account_kind"].isNull)
        #expect(saved["agent"]["provider"].isNull)
        #expect(saved["agent"]["version"].text == "stable")
    }

    @Test
    func accountConnectionSettingsAreDiscoverableWithoutHidingTheLegacyTokenPane() {
        #expect(NativeSettingsPolicy.sections(matching: "ChatGPT login").map(\.id) == ["accounts"])
        #expect(NativeSettingsPolicy.sections(matching: "device code").map(\.id) == ["accounts"])
        #expect(NativeSettingsPolicy.catalog.contains { $0.id == "account" })
    }
}
