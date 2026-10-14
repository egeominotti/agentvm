import Foundation

enum NativeAgentSettingsPolicy {
    static func defaultsUpdate(settings: JSON, selection: JSON, version: String) -> JSON {
        var next = settings
        let custom = reusableCustom(settings: settings)
        if next["custom_cli"].isNull && !custom.isNull { next["custom_cli"] = custom }
        next["agent"] = .object(["kind": .string(NativeAgentPolicy.kind(selection)),
                                 "provider": selection["provider"], "custom": .null])
        let auth = selection["auth"].text
        if auth == "account" || (auth.isEmpty && !selection["account_kind"].text.isEmpty) {
            next["agent"]["auth"] = .string("account")
            next["agent"]["account_kind"] = selection["account_kind"]
            next["agent"]["provider"] = .null
        } else if auth == "api" {
            next["agent"]["auth"] = .string("api")
        }
        let version = version.trimmingCharacters(in: .whitespacesAndNewlines)
        next["agent"]["version"] = version.isEmpty ? .null : .string(version)
        if NativeAgentPolicy.kind(settings["agent"]) != NativeAgentPolicy.kind(selection) {
            next["model"] = .string("default")
        }
        return next
    }

    static func reusableCustom(settings: JSON) -> JSON {
        if !settings["custom_cli"].isNull { return settings["custom_cli"] }
        return NativeAgentPolicy.kind(settings["agent"]) == "custom" ? settings["agent"]["custom"] : .null
    }

    static func providerUpdate(config: JSON, token: String? = nil) -> JSON {
        var body: [String: JSON] = ["config": .object([
            "endpoint": .string(config["endpoint"].text.trimmingCharacters(in: .whitespacesAndNewlines)),
            "protocol": config["protocol"], "requires_key": config["requires_key"]
        ])]
        if let token = token?.trimmingCharacters(in: .whitespacesAndNewlines), !token.isEmpty {
            body["token"] = .string(token)
        }
        return .object(body)
    }

    static func providerBlocker(endpoint: String, protocolID: String) -> String? {
        guard ["openai", "anthropic", "google"].contains(protocolID) else { return "Choose a supported API protocol." }
        let endpoint = endpoint.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let url = URLComponents(string: endpoint), ["http", "https"].contains((url.scheme ?? "").lowercased()),
              let host = url.host, !host.isEmpty else { return "Enter an absolute HTTP or HTTPS endpoint." }
        if !(url.user ?? "").isEmpty || url.password != nil || url.query != nil || url.fragment != nil {
            return "The endpoint cannot contain credentials, a query or a fragment."
        }
        return nil
    }

    static func customValue(name: String, install: String, interactive: String, headless: String, resume: String) -> JSON {
        func optional(_ text: String) -> JSON {
            let value = text.trimmingCharacters(in: .whitespacesAndNewlines)
            return value.isEmpty ? .null : .string(value)
        }
        return .object(["name": .string(name.trimmingCharacters(in: .whitespacesAndNewlines)),
                        "install": optional(install),
                        "interactive": .string(interactive.trimmingCharacters(in: .whitespacesAndNewlines)),
                        "headless": optional(headless), "resume": optional(resume)])
    }
}
