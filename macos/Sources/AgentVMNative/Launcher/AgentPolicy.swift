import Foundation

struct NativeProviderChoice: Identifiable, Equatable, Sendable {
    let id: String
    let name: String
    let available: Bool
}

enum NativeAgentPolicy {
    static func kind(_ selection: JSON) -> String { selection["kind"].text.isEmpty ? "claude" : selection["kind"].text }

    static func displayName(_ selection: JSON, catalog: JSON) -> String {
        let kind = kind(selection)
        let customName = selection["custom"]["name"].text.trimmingCharacters(in: .whitespacesAndNewlines)
        if kind == "custom" && !customName.isEmpty { return customName }
        if let descriptor = choices(catalog: catalog).first(where: { $0["kind"].text == kind }), !descriptor["name"].text.isEmpty {
            return descriptor["name"].text
        }
        let names = ["claude": "Claude Code", "codex": "Codex", "gemini": "Gemini CLI", "opencode": "OpenCode", "qwen": "Qwen Code",
                     "copilot": "GitHub Copilot CLI", "kilo": "Kilo CLI", "aider": "Aider", "goose": "Goose", "custom": "Custom CLI"]
        return names[kind] ?? kind.replacingOccurrences(of: "_", with: " ").replacingOccurrences(of: "-", with: " ").capitalized
    }

    static func choices(catalog: JSON) -> [JSON] {
        if catalog.isNull {
            return [.object(["kind": .string("claude"), "name": .string("Claude Code"), "headless": .bool(true),
                             "resume": .bool(true), "requires_model": .bool(false)])]
        }
        return catalog["agents"].items.filter { !$0["kind"].text.isEmpty }
    }

    static func defaultProvider(kind: String) -> String {
        kind == "claude" || kind == "custom" ? "" : kind == "gemini" ? "google" : "openai"
    }

    static func providers(kind: String, catalog: JSON) -> [JSON] {
        let allowed = choices(catalog: catalog).first { $0["kind"].text == kind }?["providers"].items.map(\.text) ?? []
        return catalog["providers"].items.filter {
            allowed.contains($0["id"].text) && supports(kind: kind, protocol: $0["config"]["protocol"].text)
        }
    }

    static func providerChoices(selection: JSON, catalog: JSON) -> [NativeProviderChoice] {
        let kind = kind(selection)
        var choices = providers(kind: kind, catalog: catalog).map { NativeProviderChoice(id: $0["id"].text, name: $0["name"].text, available: true) }
        let selected = NativeAgentAuthentication.provider(selection: selection, catalog: catalog)
        if !selected.isEmpty && !choices.contains(where: { $0.id == selected }) {
            let provider = catalog["providers"].items.first { $0["id"].text == selected }
            let name = provider?["name"].text ?? ""
            choices.append(NativeProviderChoice(id: selected, name: name.isEmpty ? selected.capitalized : name, available: false))
        }
        return choices
    }

    static func configurationBlocker(selection: JSON, catalog: JSON) -> String? {
        let kind = catalog.isNull ? "claude" : kind(selection)
        if !catalog.isNull && !choices(catalog: catalog).contains(where: { $0["kind"].text == kind }) {
            return "Choose a coding CLI supported by this server."
        }
        if let error = NativeAgentAuthentication.configurationBlocker(selection: selection, catalog: catalog) { return error }
        let providerID = NativeAgentAuthentication.provider(selection: selection, catalog: catalog)
        if !catalog.isNull && !providerID.isEmpty {
            guard let provider = providers(kind: kind, catalog: catalog).first(where: { $0["id"].text == providerID }) else {
                return "Choose a provider with a protocol supported by this CLI."
            }
            if !validEndpoint(provider["config"]["endpoint"].text) {
                return "Configure the \(provider["name"].text) endpoint in Settings → Providers."
            }
        }
        if kind == "custom" {
            if let error = customBlocker(selection["custom"], interactive: true) { return error }
            // The provider is preserved for later headless launches and snapshot resumes too.
            let templates = ["interactive", "headless", "resume"].map { selection["custom"][$0].text }
            if providerID.isEmpty && templates.contains(where: { $0.contains("{endpoint}") }) {
                return "Choose an inference provider for commands that use {endpoint}."
            }
            if providerID.isEmpty && templates.contains(where: { $0.contains("{model}") }) {
                return "Choose an inference provider for automatic model resolution."
            }
        }
        let version = selection["version"].text.trimmingCharacters(in: .whitespacesAndNewlines)
        if !version.isEmpty && !validVersion(version, kind: kind) {
            return "Choose latest or an exact version such as 1.2.3\(kind == "claude" ? ", or stable for Claude Code" : "")."
        }
        return nil
    }

    static func changingKind(_ selection: JSON, to kind: String) -> JSON {
        guard self.kind(selection) != kind else { return selection }
        var fields: [String: JSON] = ["kind": .string(kind)]
        let provider = defaultProvider(kind: kind)
        if !provider.isEmpty { fields["provider"] = .string(provider) }
        return .object(fields)
    }

    static func initialCustom(settings: JSON) -> JSON {
        NativeAgentSettingsPolicy.reusableCustom(settings: settings)
    }

    static func readiness(selection: JSON, catalog: JSON, legacyToken: Bool, interactive: Bool) -> String? {
        if catalog.isNull { return legacyToken ? nil : "Connect your Claude account in Settings → Claude account." }
        if let error = configurationBlocker(selection: selection, catalog: catalog) { return error }
        let kind = kind(selection)
        guard let descriptor = choices(catalog: catalog).first(where: { $0["kind"].text == kind }) else {
            return "Choose a coding CLI supported by this server."
        }
        if !interactive && !descriptor["headless"].flag { return "This CLI does not support automatic tasks." }
        let account = NativeAgentAuthentication.supported(catalog: catalog) && NativeAgentAuthentication.auth(selection: selection, catalog: catalog) == "account"
        if let error = NativeAgentAuthentication.readiness(selection: selection, catalog: catalog) { return error }
        let providerID = NativeAgentAuthentication.provider(selection: selection, catalog: catalog)
        if providerID.isEmpty && !account {
            if kind == "claude" && !catalog["claude_token"].flag {
                return "Connect your Claude account in Settings → Claude account, or choose an API provider."
            }
        } else if !account {
            guard let provider = providers(kind: kind, catalog: catalog).first(where: { $0["id"].text == providerID }) else {
                return "Choose a provider with a protocol supported by this CLI."
            }
            if provider["config"]["requires_key"].flag && !provider["has_key"].flag {
                return "Add the \(provider["name"].text) API key in Settings → Providers."
            }
        }
        if kind == "custom", let error = customBlocker(selection["custom"], interactive: interactive) { return error }
        return nil
    }

    static func customBlocker(_ custom: JSON, interactive: Bool) -> String? {
        NativeCustomCLIValidation.blocker(custom, interactive: interactive)
    }

    static func launchBody(base: JSON, selection: JSON, version: String, supported: Bool, catalog: JSON = .null) -> JSON {
        var body = base.fields
        body.removeValue(forKey: "agent")
        body.removeValue(forKey: "model")
        let version = version.trimmingCharacters(in: .whitespacesAndNewlines)
        if !supported {
            body["claude_version"] = version.isEmpty ? .null : .string(version)
            return .object(body)
        }
        let kind = kind(selection)
        var agent: [String: JSON] = ["kind": .string(kind)]
        if NativeAgentAuthentication.supported(catalog: catalog) {
            let mode = NativeAgentAuthentication.auth(selection: selection, catalog: catalog)
            agent["auth"] = .string(mode)
            if mode == "account" { agent["account_kind"] = .string(NativeAgentAuthentication.accountKind(selection: selection, catalog: catalog)) }
            else if !selection["provider"].text.isEmpty { agent["provider"] = selection["provider"] }
        } else if !selection["provider"].text.isEmpty { agent["provider"] = selection["provider"] }
        if !version.isEmpty { agent["version"] = .string(version) }
        if kind == "custom", !selection["custom"].isNull { agent["custom"] = selection["custom"] }
        body["agent"] = .object(agent)
        body.removeValue(forKey: "claude_version")
        return .object(body)
    }

    private static func supports(kind: String, protocol value: String) -> Bool {
        guard ["openai", "anthropic", "google"].contains(value) else { return false }
        switch kind {
        case "claude": return value == "anthropic"
        case "codex", "goose": return value == "openai"
        case "qwen", "copilot": return value == "openai" || value == "anthropic"
        case "gemini": return value == "google"
        default: return true
        }
    }

    private static func validEndpoint(_ endpoint: String) -> Bool {
        guard let url = URLComponents(string: endpoint), ["https", "http"].contains((url.scheme ?? "").lowercased()),
              let host = url.host, !host.isEmpty else { return false }
        return (url.user ?? "").isEmpty && url.password == nil && url.query == nil && url.fragment == nil
    }

    private static func validVersion(_ value: String, kind: String) -> Bool {
        if value == "latest" || (kind == "claude" && value == "stable") { return true }
        let parts = value.split(separator: "-", maxSplits: 1, omittingEmptySubsequences: false)
        let numbers = parts[0].split(separator: ".", omittingEmptySubsequences: false)
        guard numbers.count == 3, numbers.allSatisfy({ !$0.isEmpty && $0.utf8.allSatisfy { (48...57).contains($0) } }) else { return false }
        return parts.count == 1 || (!parts[1].isEmpty && parts[1].utf8.allSatisfy { (48...57).contains($0) || (65...90).contains($0) || (97...122).contains($0) || $0 == 46 })
    }
}
