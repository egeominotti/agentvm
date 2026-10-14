import Testing
@testable import AgentVMNative

@MainActor
@Suite("Native authentication form transitions")
struct NativeAgentAuthenticationBindingTests {
    @Test("Switching account and API through the real form binding keeps version and restores its provider")
    func modes() {
        let draft = AgentFieldDraft(catalog: NativeAgentAuthenticationTests().catalog)
        draft.fields.authenticationSelection.wrappedValue = "account"
        #expect(draft.selection["account_kind"].text == "claude")
        #expect(draft.selection["provider"].isNull)
        #expect(draft.version == "2.1.294")
        #expect(draft.customVersion)
        draft.fields.authenticationSelection.wrappedValue = "api"
        #expect(draft.selection["provider"].text == "anthropic")
        #expect(draft.selection["account_kind"].isNull)
        #expect(draft.version == "2.1.294")
    }

    @Test("Choosing a CLI through the form adopts its connected eligible account")
    func kind() {
        let draft = AgentFieldDraft(catalog: NativeAgentAuthenticationTests().catalog)
        draft.fields.kindSelection.wrappedValue = "codex"
        #expect(draft.selection["auth"].text == "account")
        #expect(draft.selection["account_kind"].text == "chatgpt")
        #expect(draft.selection["provider"].isNull)
        #expect(draft.version.isEmpty)
        #expect(!draft.customVersion)
        draft.fields.authenticationSelection.wrappedValue = "api"
        #expect(draft.selection["provider"].text == "openai")
        #expect(draft.custom["interactive"].text == "my-tool --prompt {prompt}")
    }
}
