import Testing
import SwiftUI
@testable import AgentVMNative

@Suite("Native multi-CLI launcher policy")
struct NativeAgentPolicyTests {
    private let catalog = JSON.object([
        "claude_token": .bool(false),
        "agents": .array([
            .object(["kind": .string("claude"), "name": .string("Claude Code"), "providers": .array([.string("anthropic"), .string("custom")]),
                     "headless": .bool(true), "resume": .bool(true), "requires_model": .bool(false)]),
            .object(["kind": .string("codex"), "name": .string("Codex"), "providers": .array([.string("openai"), .string("local"), .string("custom")]),
                     "headless": .bool(true), "resume": .bool(true), "requires_model": .bool(false)]),
            .object(["kind": .string("goose"), "name": .string("Goose"), "providers": .array([.string("openai"), .string("local"), .string("custom")]),
                     "headless": .bool(true), "resume": .bool(true), "requires_model": .bool(true)]),
            .object(["kind": .string("gemini"), "name": .string("Gemini CLI"), "providers": .array([.string("google"), .string("custom")]),
                     "headless": .bool(true), "resume": .bool(true), "requires_model": .bool(false)]),
            .object(["kind": .string("custom"), "name": .string("Custom CLI"), "providers": .array([.string("openai"), .string("local"), .string("custom")]),
                     "headless": .bool(true), "resume": .bool(true), "requires_model": .bool(false)])
        ]),
        "providers": .array([
            .object(["id": .string("openai"), "name": .string("OpenAI"), "has_key": .bool(true),
                     "config": .object(["endpoint": .string("https://api.openai.com/v1"), "protocol": .string("openai"), "requires_key": .bool(true)])]),
            .object(["id": .string("anthropic"), "name": .string("Anthropic"), "has_key": .bool(false),
                     "config": .object(["endpoint": .string("https://api.anthropic.com"), "protocol": .string("anthropic"), "requires_key": .bool(true)])]),
            .object(["id": .string("google"), "name": .string("Google Gemini"), "has_key": .bool(true),
                     "config": .object(["endpoint": .string("https://generativelanguage.googleapis.com"), "protocol": .string("google"), "requires_key": .bool(true)])]),
            .object(["id": .string("local"), "name": .string("Local"), "has_key": .bool(false),
                     "config": .object(["endpoint": .string("http://127.0.0.1:11434/v1"), "protocol": .string("openai"), "requires_key": .bool(false)])]),
            .object(["id": .string("custom"), "name": .string("Custom"), "has_key": .bool(true),
                     "config": .object(["endpoint": .string("https://inference.example/v1"), "protocol": .string("anthropic"), "requires_key": .bool(true)])])
        ])
    ])

    @Test("Missing selections identify Claude and custom or unknown agents keep their real names")
    func displayNames() {
        #expect(NativeAgentPolicy.kind(.null) == "claude")
        #expect(NativeAgentPolicy.displayName(.null, catalog: .null) == "Claude Code")
        #expect(NativeAgentPolicy.displayName(.object(["kind": .string("codex")]), catalog: catalog) == "Codex")
        #expect(NativeAgentPolicy.displayName(.object(["kind": .string("custom"), "custom": .object(["name": .string("My Tool")])]), catalog: catalog) == "My Tool")
        #expect(NativeAgentPolicy.displayName(.object(["kind": .string("future_cli")]), catalog: catalog) == "Future Cli")
    }

    @Test("Repository validation accepts ready API agents without applying the Claude-token requirement twice")
    func repositoryGate() {
        let status = JSON.object(["golden": .bool(true), "token": .bool(false)])
        let check = JSON.object(["ok": .bool(true)])
        #expect(NativeLauncherLogic.blocker(status: status, connected: true, repo: "/tmp/work", check: check,
                                          interactive: true, prompt: "", requireClaudeToken: false, agentName: "Codex") == nil)
        #expect(NativeLauncherLogic.blocker(status: status, connected: true, repo: "/tmp/work", check: check,
                                          interactive: true, prompt: "") != nil)
    }

    @Test("An unavailable catalog preserves the legacy Claude request without agent fields")
    func legacyRequest() {
        let base = JSON.object(["repo_path": .string("/tmp/work"), "interactive": .bool(true), "cpus": .number(4)])
        let body = NativeAgentPolicy.launchBody(base: base, selection: .object(["kind": .string("codex")]),
                                              version: "2.1.294", supported: false)
        #expect(body == .object(["repo_path": .string("/tmp/work"), "interactive": .bool(true), "cpus": .number(4),
                                "claude_version": .string("2.1.294")]))
        #expect(NativeAgentPolicy.choices(catalog: .null).map { $0["kind"].text } == ["claude"])
    }

    @Test("Changing agent clears previous provider, version and commands")
    func changingAgent() {
        let prior = JSON.object(["kind": .string("claude"), "provider": .string("anthropic"),
                                 "version": .string("stable"), "custom": .object(["name": .string("old")])])
        let changed = NativeAgentPolicy.changingKind(prior, to: "codex")
        #expect(changed["kind"].text == "codex")
        #expect(changed["provider"].text == "openai")
        #expect(changed["version"].isNull)
        #expect(changed["custom"].isNull)
        #expect(NativeAgentPolicy.defaultProvider(kind: "gemini") == "google")
        #expect(NativeAgentPolicy.defaultProvider(kind: "custom") == "")
    }

    @MainActor
    @Test("Reselecting the current CLI preserves its version and reusable commands")
    func sameKindBinding() {
        let draft = AgentFieldDraft(catalog: catalog)
        draft.fields.kindSelection.wrappedValue = "claude"
        #expect(draft.version == "2.1.294")
        #expect(draft.customVersion)
        #expect(draft.selection["provider"].text == "anthropic")
        #expect(draft.custom["interactive"].text == "my-tool --prompt {prompt}")
    }

    @MainActor
    @Test("Choosing another CLI clears only incompatible draft fields")
    func changedKindBinding() {
        let draft = AgentFieldDraft(catalog: catalog)
        draft.fields.kindSelection.wrappedValue = "codex"
        #expect(draft.selection["kind"].text == "codex")
        #expect(draft.selection["provider"].text == "openai")
        #expect(draft.version == "")
        #expect(!draft.customVersion)
        #expect(draft.custom["interactive"].text == "my-tool --prompt {prompt}")
    }

    @Test("Codex and credential-free endpoints launch without a Claude subscription")
    func authentication() {
        #expect(NativeAgentPolicy.readiness(selection: .object(["kind": .string("codex")]), catalog: catalog,
                                          legacyToken: false, interactive: true) == nil)
        #expect(NativeAgentPolicy.readiness(selection: .object(["kind": .string("codex"), "provider": .string("local")]), catalog: catalog,
                                          legacyToken: false, interactive: true) == nil)
        #expect(NativeAgentPolicy.readiness(selection: .object(["kind": .string("claude")]), catalog: catalog,
                                          legacyToken: false, interactive: true) != nil)
        #expect(NativeAgentPolicy.readiness(selection: .object(["kind": .string("claude"), "provider": .string("anthropic")]), catalog: catalog,
                                          legacyToken: true, interactive: true)?.contains("Anthropic") == true)
        var subscribed = catalog
        subscribed["claude_token"] = .bool(true)
        #expect(NativeAgentPolicy.readiness(selection: .object(["kind": .string("claude")]), catalog: subscribed,
                                          legacyToken: false, interactive: true) == nil)
    }

    @Test("Configured provider protocols control choices and reject incompatible overrides")
    func providerProtocols() {
        #expect(NativeAgentPolicy.providers(kind: "codex", catalog: catalog).map { $0["id"].text } == ["openai", "local"])
        #expect(NativeAgentPolicy.providers(kind: "claude", catalog: catalog).map { $0["id"].text } == ["anthropic", "custom"])
        #expect(NativeAgentPolicy.readiness(selection: .object(["kind": .string("codex"), "provider": .string("custom")]), catalog: catalog,
                                          legacyToken: false, interactive: true) != nil)
        var invalid = catalog
        var providers = invalid["providers"].items
        providers[0]["config"]["endpoint"] = .string("https://user:password@example.com/v1")
        invalid["providers"] = .array(providers)
        #expect(NativeAgentPolicy.readiness(selection: .object(["kind": .string("codex")]), catalog: invalid,
                                          legacyToken: false, interactive: true) != nil)
    }

    @Test("Launch readiness leaves model resolution to the backend")
    func explicitModels() {
        let selection = JSON.object(["kind": .string("goose")])
        for interactive in [true, false] {
            #expect(NativeAgentPolicy.readiness(selection: selection, catalog: catalog,
                                              legacyToken: false, interactive: interactive) == nil)
        }
    }

    @Test("Non-Claude task payloads carry their own version and omit default models")
    func modernRequest() {
        let base = JSON.object(["repo_path": .string("/tmp/work"), "interactive": .bool(true), "claude_version": .string("stable")])
        let body = NativeAgentPolicy.launchBody(base: base, selection: .object(["kind": .string("codex"), "provider": .string("openai")]),
                                              version: "1.2.3", supported: true)
        #expect(body == .object(["repo_path": .string("/tmp/work"), "interactive": .bool(true),
                                "agent": .object(["kind": .string("codex"), "provider": .string("openai"), "version": .string("1.2.3")])]))
        let explicit = NativeAgentPolicy.launchBody(base: base, selection: .object(["kind": .string("goose")]),
                                                   version: "", supported: true)
        #expect(explicit.fields["model"] == nil)
        #expect(explicit["claude_version"].isNull)
        #expect(explicit["agent"]["version"].isNull)
    }

    @Test("Custom CLI templates retain literal placeholders and automatic tasks need a headless command")
    func customCommands() {
        let custom = JSON.object(["name": .string("My CLI"), "install": .string("curl https://example.com/install | sh"),
                                  "interactive": .string("tool --prompt '{prompt}' --model {model}")])
        #expect(NativeAgentPolicy.customBlocker(custom, interactive: true) == nil)
        #expect(NativeAgentPolicy.customBlocker(custom, interactive: false) != nil)
        var selection = JSON.object(["kind": .string("custom"), "custom": custom])
        #expect(NativeAgentPolicy.readiness(selection: selection, catalog: catalog,
                                          legacyToken: false, interactive: true) != nil)
        selection["provider"] = .string("local")
        #expect(NativeAgentPolicy.readiness(selection: selection, catalog: catalog,
                                          legacyToken: false, interactive: true) == nil)
        for command in ["tool; rm -rf work", "tool $(echo danger)", "tool --prompt 'unfinished", "--help", "{model}"] {
            var invalid = custom
            invalid["interactive"] = .string(command)
            #expect(NativeAgentPolicy.customBlocker(invalid, interactive: true) != nil)
        }
        let body = NativeAgentPolicy.launchBody(base: .object([:]), selection: selection, version: "", supported: true)
        #expect(body["agent"]["custom"] == custom)
    }

    @Test("Custom endpoint placeholders require a provider, while providerless local commands remain valid")
    func customEndpoint() {
        let custom = JSON.object(["name": .string("Tool"), "interactive": .string("tool --endpoint {endpoint}")])
        var selection = JSON.object(["kind": .string("custom"), "custom": custom])
        #expect(NativeAgentPolicy.readiness(selection: selection, catalog: catalog, legacyToken: false,
                                          interactive: true) != nil)
        selection["provider"] = .string("local")
        #expect(NativeAgentPolicy.readiness(selection: selection, catalog: catalog, legacyToken: false,
                                          interactive: true) == nil)
        selection["provider"] = .null
        selection["custom"]["interactive"] = .string("tool")
        #expect(NativeAgentPolicy.readiness(selection: selection, catalog: catalog, legacyToken: false,
                                          interactive: true) == nil)
        selection["custom"]["resume"] = .string("tool resume --endpoint {endpoint}")
        #expect(NativeAgentPolicy.readiness(selection: selection, catalog: catalog, legacyToken: false,
                                          interactive: true) != nil)
    }

    @Test("Default configuration can be saved before credentials or launch-required models are supplied")
    func configurationOnly() {
        let claude = JSON.object(["kind": .string("claude"), "provider": .string("anthropic")])
        #expect(NativeAgentPolicy.configurationBlocker(selection: claude, catalog: catalog) == nil)
        let goose = JSON.object(["kind": .string("goose")])
        #expect(NativeAgentPolicy.configurationBlocker(selection: goose, catalog: catalog) == nil)
        let unsupported = JSON.object(["kind": .string("codex"), "provider": .string("custom")])
        #expect(NativeAgentPolicy.configurationBlocker(selection: unsupported, catalog: catalog) != nil)
        var wrongVersion = goose
        wrongVersion["version"] = .string("stable")
        #expect(NativeAgentPolicy.configurationBlocker(selection: wrongVersion, catalog: catalog) != nil)
    }

    @Test("An incompatible saved provider stays visible as unavailable instead of losing its picker tag")
    func unavailableProviderChoice() {
        let selection = JSON.object(["kind": .string("codex"), "provider": .string("custom")])
        let choices = NativeAgentPolicy.providerChoices(selection: selection, catalog: catalog)
        #expect(choices.map(\.id) == ["openai", "local", "custom"])
        #expect(choices.first { $0.id == "custom" }?.available == false)
        #expect(choices.first { $0.id == "custom" }?.name == "Custom")
        #expect(NativeAgentPolicy.providers(kind: "codex", catalog: catalog).map { $0["id"].text } == ["openai", "local"])
    }

    @Test("Provider URL schemes are case insensitive in saved defaults and launch checks")
    func endpointScheme() {
        var configured = catalog
        var providers = configured["providers"].items
        providers[0]["config"]["endpoint"] = .string("HTTPS://api.openai.com/v1")
        configured["providers"] = .array(providers)
        let selection = JSON.object(["kind": .string("codex")])
        #expect(NativeAgentPolicy.configurationBlocker(selection: selection, catalog: configured) == nil)
        #expect(NativeAgentPolicy.readiness(selection: selection, catalog: configured, legacyToken: false,
                                          interactive: true) == nil)
    }

    @Test("Custom resume commands rely on backend model resolution")
    func resumeModel() {
        let custom = JSON.object(["name": .string("Tool"), "interactive": .string("tool"), "resume": .string("tool resume --model {model}")])
        let selection = JSON.object(["kind": .string("custom"), "provider": .string("local"), "custom": custom])
        #expect(NativeAgentPolicy.readiness(selection: selection, catalog: catalog, legacyToken: false, interactive: true) == nil)
    }

    @Test("New launches use updated reusable custom commands and retain legacy inline-only defaults")
    func initialCustom() {
        let old = JSON.object(["name": .string("Old CLI"), "interactive": .string("old-cli")])
        let current = JSON.object(["name": .string("New CLI"), "interactive": .string("new-cli")])
        let agent = JSON.object(["kind": .string("custom"), "custom": old])
        #expect(NativeAgentPolicy.initialCustom(settings: .object(["agent": agent, "custom_cli": current])) == current)
        #expect(NativeAgentPolicy.initialCustom(settings: .object(["agent": agent])) == old)
    }
}

@MainActor
final class AgentFieldDraft {
    let catalog: JSON
    var selection = JSON.object(["kind": .string("claude"), "provider": .string("anthropic")])
    var version = "2.1.294"
    var custom = JSON.object(["name": .string("My Tool"), "interactive": .string("my-tool --prompt {prompt}")])
    var customVersion = true
    var rememberedProviders: [String: String] = [:]

    init(catalog: JSON) { self.catalog = catalog }

    var fields: NativeAgentFields {
        NativeAgentFields(catalog: catalog, supported: true, releases: [], selection: binding(\.selection),
                          version: binding(\.version), custom: binding(\.custom),
                          customVersion: binding(\.customVersion), rememberedProviders: binding(\.rememberedProviders),
                          configureProviders: {}, configureAccounts: {})
    }

    private func binding<Value>(_ path: ReferenceWritableKeyPath<AgentFieldDraft, Value>) -> Binding<Value> {
        Binding(get: { self[keyPath: path] }, set: { self[keyPath: path] = $0 })
    }
}
