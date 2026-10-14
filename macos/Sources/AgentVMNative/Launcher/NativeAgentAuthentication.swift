enum NativeAgentAuthentication {
    static func supported(catalog: JSON) -> Bool {
        if case .array = catalog["accounts"] { return true }
        return false
    }

    static func auth(selection: JSON, catalog: JSON) -> String {
        let legacy = NativeAgentPolicy.kind(selection) == "claude" && selection["provider"].text.isEmpty ? "account" : "api"
        guard supported(catalog: catalog) else { return legacy }
        if !selection["auth"].text.isEmpty { return selection["auth"].text }
        return selection["account_kind"].text.isEmpty ? legacy : "account"
    }

    static func accountChoices(selection: JSON, catalog: JSON) -> [JSON] {
        let eligible = accountIDs(selection: selection, catalog: catalog)
        return catalog["accounts"].items.filter { eligible.contains($0["id"].text) }
    }

    static func accountKind(selection: JSON, catalog: JSON) -> String {
        if !selection["account_kind"].text.isEmpty { return selection["account_kind"].text }
        return accountIDs(selection: selection, catalog: catalog).first ??
            (NativeAgentPolicy.kind(selection) == "claude" ? "claude" : "")
    }

    static func provider(selection: JSON, catalog: JSON) -> String {
        let kind = NativeAgentPolicy.kind(selection)
        if supported(catalog: catalog) {
            if auth(selection: selection, catalog: catalog) == "account" { return "" }
            if kind == "claude" && selection["provider"].text.isEmpty { return "anthropic" }
        }
        return selection["provider"].text.isEmpty ? NativeAgentPolicy.defaultProvider(kind: kind) : selection["provider"].text
    }

    static func changingAuth(selection: JSON, to mode: String, catalog: JSON, rememberedProvider: String? = nil) -> JSON {
        guard supported(catalog: catalog) else { return selection }
        var fields = selection.fields
        fields["auth"] = .string(mode)
        if mode == "account" {
            fields.removeValue(forKey: "provider")
            let choices = accountChoices(selection: selection, catalog: catalog)
            let prior = selection["account_kind"].text
            let selected = choices.first { $0["id"].text == prior } ?? choices.first { $0["connected"].flag } ?? choices.first
            fields["account_kind"] = selected?["id"] ?? .string(accountKind(selection: selection, catalog: catalog))
        } else if mode == "api" {
            fields.removeValue(forKey: "account_kind")
            let kind = NativeAgentPolicy.kind(selection)
            let choices = NativeAgentPolicy.providers(kind: kind, catalog: catalog).map { $0["id"].text }
            let defaultID = kind == "claude" ? "anthropic" : NativeAgentPolicy.defaultProvider(kind: kind)
            let candidates = [rememberedProvider ?? "", selection["provider"].text, defaultID]
            let selected = candidates.first { !$0.isEmpty && choices.contains($0) } ??
                (kind == "custom" && candidates.allSatisfy(\.isEmpty) ? "" : choices.first ?? defaultID)
            if selected.isEmpty { fields.removeValue(forKey: "provider") }
            else { fields["provider"] = .string(selected) }
        }
        return .object(fields)
    }

    static func changingAccount(selection: JSON, to account: String) -> JSON {
        var fields = selection.fields
        fields["auth"] = .string("account")
        fields["account_kind"] = .string(account)
        fields.removeValue(forKey: "provider")
        return .object(fields)
    }

    static func changingKind(selection: JSON, to kind: String, catalog: JSON) -> JSON {
        guard kind != NativeAgentPolicy.kind(selection) else { return selection }
        var next = NativeAgentPolicy.changingKind(selection, to: kind)
        guard supported(catalog: catalog) else { return next }
        let prior = selection["account_kind"].text
        if auth(selection: selection, catalog: catalog) == "account" && accountIDs(selection: next, catalog: catalog).contains(prior) {
            next["account_kind"] = .string(prior)
            return changingAuth(selection: next, to: "account", catalog: catalog)
        }
        if accountChoices(selection: next, catalog: catalog).contains(where: { $0["connected"].flag }) || kind == "claude" {
            return changingAuth(selection: next, to: "account", catalog: catalog)
        }
        return changingAuth(selection: next, to: "api", catalog: catalog)
    }

    static func configurationBlocker(selection: JSON, catalog: JSON) -> String? {
        guard supported(catalog: catalog) else { return nil }
        let mode = auth(selection: selection, catalog: catalog)
        guard ["account", "api"].contains(mode) else { return "Choose account or API authentication." }
        if mode == "api" {
            return selection["account_kind"].text.isEmpty ? nil : "API authentication cannot select an account."
        }
        if !selection["provider"].text.isEmpty { return "Account authentication cannot select an API provider." }
        let account = accountKind(selection: selection, catalog: catalog)
        guard accountIDs(selection: selection, catalog: catalog).contains(account) else {
            return "Choose an account supported by this coding CLI."
        }
        guard let status = accountChoices(selection: selection, catalog: catalog).first(where: { $0["id"].text == account }),
              !status["name"].text.isEmpty, case .bool = status["connected"] else {
            return "This account is unavailable. Refresh Settings → Accounts."
        }
        return nil
    }

    static func readiness(selection: JSON, catalog: JSON) -> String? {
        guard supported(catalog: catalog), auth(selection: selection, catalog: catalog) == "account" else { return nil }
        if let error = configurationBlocker(selection: selection, catalog: catalog) { return error }
        let account = accountChoices(selection: selection, catalog: catalog).first { $0["id"].text == accountKind(selection: selection, catalog: catalog) }
        let detail = account?["error"].text ?? ""
        if !detail.isEmpty { return "\(detail). Connect the account in Settings → Accounts." }
        return account?["connected"].flag == true ? nil : "Connect \(account?["name"].text ?? "your account") in Settings → Accounts."
    }

    private static func accountIDs(selection: JSON, catalog: JSON) -> [String] {
        NativeAgentPolicy.choices(catalog: catalog).first { $0["kind"].text == NativeAgentPolicy.kind(selection) }?["accounts"].items.map(\.text) ?? []
    }
}
