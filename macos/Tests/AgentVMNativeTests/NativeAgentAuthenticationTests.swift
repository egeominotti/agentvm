import Testing
@testable import AgentVMNative

@Suite("Native agent authentication")
struct NativeAgentAuthenticationTests {
    var catalog: JSON {
        let eligible = ["claude": ["claude"], "codex": ["chatgpt"], "gemini": ["google"], "opencode": ["chatgpt"],
                        "qwen": [], "copilot": ["copilot"], "kilo": ["kilo", "chatgpt"], "aider": [], "goose": ["claude", "chatgpt"], "custom": []]
        return .object([
            "claude_token": .bool(false),
            "agents": .array(eligible.sorted { $0.key < $1.key }.map { kind, accounts in .object([
                "kind": .string(kind), "name": .string(kind), "providers": .array([.string("anthropic"), .string("openai"), .string("local")]),
                "headless": .bool(true), "resume": .bool(true), "requires_model": .bool(false), "accounts": .array(accounts.map(JSON.string))
            ]) }),
            "providers": .array([
                .object(["id": .string("anthropic"), "name": .string("Anthropic"), "has_key": .bool(false),
                         "config": .object(["endpoint": .string("https://api.anthropic.com"), "protocol": .string("anthropic"), "requires_key": .bool(true)])]),
                .object(["id": .string("openai"), "name": .string("OpenAI"), "has_key": .bool(false),
                         "config": .object(["endpoint": .string("https://api.openai.com/v1"), "protocol": .string("openai"), "requires_key": .bool(true)])]),
                .object(["id": .string("local"), "name": .string("Local"), "has_key": .bool(false),
                         "config": .object(["endpoint": .string("http://127.0.0.1:11434/v1"), "protocol": .string("openai"), "requires_key": .bool(false)])])
            ]),
            "accounts": .array([
                .object(["id": .string("claude"), "name": .string("Claude account"), "connected": .bool(false), "source": .null, "error": .null]),
                .object(["id": .string("chatgpt"), "name": .string("ChatGPT account"), "connected": .bool(true), "source": .string("Keychain"), "error": .null]),
                .object(["id": .string("kilo"), "name": .string("Kilo account and credits"), "connected": .bool(false), "source": .null, "error": .null])
            ])
        ])
    }

    @Test("Connected account authentication does not require an unrelated API key")
    func accountReadiness() {
        let selection = JSON.object(["kind": .string("codex"), "auth": .string("account"), "account_kind": .string("chatgpt")])
        #expect(NativeAgentPolicy.configurationBlocker(selection: selection, catalog: catalog) == nil)
        #expect(NativeAgentPolicy.readiness(selection: selection, catalog: catalog, legacyToken: false, interactive: true) == nil)
    }

    @Test("Disconnected accounts and mutually exclusive or ineligible selections block launch")
    func invalidAccounts() {
        for selection in [
            JSON.object(["kind": .string("codex"), "auth": .string("account"), "account_kind": .string("chatgpt"), "provider": .string("openai")]),
            .object(["kind": .string("codex"), "auth": .string("api"), "account_kind": .string("chatgpt"), "provider": .string("local")]),
            .object(["kind": .string("qwen"), "auth": .string("account"), "account_kind": .string("chatgpt")]),
            .object(["kind": .string("codex"), "auth": .string("invalid"), "provider": .string("local")])
        ] { #expect(NativeAgentPolicy.configurationBlocker(selection: selection, catalog: catalog) != nil) }
        let disconnected = JSON.object(["kind": .string("claude"), "auth": .string("account"), "account_kind": .string("claude")])
        #expect(NativeAgentPolicy.configurationBlocker(selection: disconnected, catalog: catalog) == nil)
        #expect(NativeAgentPolicy.readiness(selection: disconnected, catalog: catalog, legacyToken: true, interactive: true) != nil)
    }

    @Test("A selected account is retained in modern task payloads without provider or model leakage")
    func accountPayload() {
        let selection = JSON.object(["kind": .string("codex"), "auth": .string("account"), "account_kind": .string("chatgpt"), "provider": .string("openai")])
        let base = JSON.object(["repo_path": .string("/tmp/repo"), "model": .string("saved-wrong-model")])
        let body = NativeAgentPolicy.launchBody(base: base, selection: selection, version: "1.2.3", supported: true, catalog: catalog)
        #expect(body["agent"]["auth"].text == "account")
        #expect(body["agent"]["account_kind"].text == "chatgpt")
        #expect(body["agent"]["provider"].isNull)
        #expect(body["model"].isNull)
    }

    @Test("Mode changes preserve CLI versions and restore a compatible remembered API provider")
    func modeTransitions() {
        let api = JSON.object(["kind": .string("codex"), "auth": .string("api"), "provider": .string("local"), "version": .string("1.2.3")])
        let account = NativeAgentAuthentication.changingAuth(selection: api, to: "account", catalog: catalog)
        #expect(account["auth"].text == "account")
        #expect(account["account_kind"].text == "chatgpt")
        #expect(account["provider"].isNull)
        #expect(account["version"].text == "1.2.3")
        let restored = NativeAgentAuthentication.changingAuth(selection: account, to: "api", catalog: catalog, rememberedProvider: "local")
        #expect(restored["provider"].text == "local")
        #expect(restored["account_kind"].isNull)
        #expect(restored["version"].text == "1.2.3")
        let incompatible = NativeAgentAuthentication.changingAuth(selection: account, to: "api", catalog: catalog, rememberedProvider: "anthropic")
        #expect(incompatible["provider"].text == "openai")
    }

    @Test("Changing CLI chooses a connected eligible account and clears the old version")
    func kindTransitions() {
        let prior = JSON.object(["kind": .string("claude"), "auth": .string("account"), "account_kind": .string("claude"), "version": .string("stable")])
        let kilo = NativeAgentAuthentication.changingKind(selection: prior, to: "kilo", catalog: catalog)
        #expect(kilo["account_kind"].text == "chatgpt")
        #expect(kilo["auth"].text == "account")
        #expect(kilo["provider"].isNull)
        #expect(kilo["version"].isNull)
        let qwen = NativeAgentAuthentication.changingKind(selection: kilo, to: "qwen", catalog: catalog)
        #expect(qwen["auth"].text == "api")
        #expect(qwen["provider"].text == "openai")
        #expect(qwen["account_kind"].isNull)
        #expect(NativeAgentAuthentication.changingKind(selection: kilo, to: "kilo", catalog: catalog) == kilo)
    }

    @Test("An eligible selected account survives a CLI change even when another account is connected")
    func preservesEligibleAccount() {
        var available = catalog
        var accounts = available["accounts"].items
        accounts[0]["connected"] = .bool(true)
        available["accounts"] = .array(accounts)
        let prior = JSON.object(["kind": .string("kilo"), "auth": .string("account"), "account_kind": .string("chatgpt"), "version": .string("1.2.3")])
        let goose = NativeAgentAuthentication.changingKind(selection: prior, to: "goose", catalog: available)
        #expect(goose["account_kind"].text == "chatgpt")
        #expect(goose["provider"].isNull)
        #expect(goose["version"].isNull)
    }

    @Test("Modern account and API payloads are exclusive and never carry models for any CLI")
    func allModernPayloads() {
        let base = JSON.object(["repo_path": .string("/tmp/repo"), "model": .string("saved-wrong-model")])
        for descriptor in catalog["agents"].items {
            let kind = descriptor["kind"].text
            let provider = kind == "claude" ? "anthropic" : kind == "gemini" ? "google" : "local"
            let api = JSON.object(["kind": .string(kind), "auth": .string("api"), "provider": .string(provider)])
            let body = NativeAgentPolicy.launchBody(base: base, selection: api, version: "1.2.3", supported: true, catalog: catalog)
            #expect(body["agent"]["auth"].text == "api")
            #expect(body["agent"]["provider"].text == provider)
            #expect(body["agent"]["account_kind"].isNull)
            #expect(body["model"].isNull)
            for account in descriptor["accounts"].items {
                let selected = JSON.object(["kind": .string(kind), "account_kind": account])
                let accountBody = NativeAgentPolicy.launchBody(base: base, selection: selected, version: "", supported: true, catalog: catalog)
                #expect(accountBody["agent"]["auth"].text == "account")
                #expect(accountBody["agent"]["account_kind"] == account)
                #expect(accountBody["agent"]["provider"].isNull)
                #expect(accountBody["model"].isNull)
            }
        }
        let clause = NativeAgentPolicy.launchBody(base: base, selection: .object(["kind": .string("claude")]), version: "", supported: true, catalog: catalog)
        #expect(clause["agent"]["auth"].text == "account")
        #expect(clause["agent"]["account_kind"].text == "claude")
    }

    @Test("Old catalogs keep implicit authentication and omit unsupported metadata")
    func legacyCatalog() {
        var older = catalog
        older["accounts"] = .null
        let selection = JSON.object(["kind": .string("codex"), "auth": .string("account"), "account_kind": .string("chatgpt"), "provider": .string("local")])
        #expect(!NativeAgentAuthentication.supported(catalog: older))
        #expect(NativeAgentAuthentication.auth(selection: selection, catalog: older) == "api")
        #expect(NativeAgentAuthentication.changingAuth(selection: selection, to: "account", catalog: older) == selection)
        let body = NativeAgentPolicy.launchBody(base: .object([:]), selection: selection, version: "", supported: true, catalog: older)
        #expect(body["agent"]["auth"].isNull)
        #expect(body["agent"]["account_kind"].isNull)
        #expect(body["agent"]["provider"].text == "local")
        let legacy = NativeAgentPolicy.launchBody(base: .object([:]), selection: selection, version: "", supported: false, catalog: catalog)
        #expect(legacy["agent"].isNull)
    }

    @Test("Existing selections infer account authentication exactly as the domain does")
    func implicitModes() {
        #expect(NativeAgentAuthentication.auth(selection: .object(["kind": .string("claude")]), catalog: catalog) == "account")
        #expect(NativeAgentAuthentication.auth(selection: .object(["kind": .string("codex")]), catalog: catalog) == "api")
        let imported = JSON.object(["kind": .string("codex"), "account_kind": .string("chatgpt")])
        #expect(NativeAgentAuthentication.auth(selection: imported, catalog: catalog) == "account")
        #expect(NativeAgentAuthentication.accountKind(selection: imported, catalog: catalog) == "chatgpt")
        let apiClaude = JSON.object(["kind": .string("claude"), "auth": .string("api")])
        #expect(NativeAgentAuthentication.provider(selection: apiClaude, catalog: catalog) == "anthropic")
    }

    @Test("Account discovery errors and missing statuses remain visible before launch")
    func accountFailures() {
        let selection = JSON.object(["kind": .string("codex"), "auth": .string("account"), "account_kind": .string("chatgpt")])
        var failed = catalog
        var accounts = failed["accounts"].items
        accounts[1]["connected"] = .bool(false)
        accounts[1]["error"] = .string("Host credential could not be read")
        failed["accounts"] = .array(accounts)
        #expect(NativeAgentPolicy.readiness(selection: selection, catalog: failed, legacyToken: true, interactive: true)?.contains("Host credential could not be read") == true)
        failed["accounts"] = .array([])
        #expect(NativeAgentPolicy.configurationBlocker(selection: selection, catalog: failed) != nil)
        accounts[1]["connected"] = .string("yes")
        failed["accounts"] = .array(accounts)
        #expect(NativeAgentPolicy.configurationBlocker(selection: selection, catalog: failed) != nil)
    }
}
