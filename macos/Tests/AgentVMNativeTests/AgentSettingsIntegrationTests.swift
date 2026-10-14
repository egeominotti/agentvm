import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native agent Settings through the real API")
struct AgentSettingsIntegrationTests {
    private static var endpoint: URL? {
        guard let raw = ProcessInfo.processInfo.environment["AGENTVM_NATIVE_AGENT_TEST_URL"],
              let url = URL(string: raw), url.scheme == "http",
              ["127.0.0.1", "localhost"].contains(url.host ?? ""),
              let port = url.port, port != 7777 else { return nil }
        return url
    }

    @MainActor
    @Test(.enabled(if: endpoint != nil))
    func publicDefaultsAndReusableCommandsPreserveProviderCredentialsAndNewConfiguration() async throws {
        let store = NativeStore(baseURL: try #require(Self.endpoint))
        let status = try await store.api.request("/api/status")
        try #require(status["running"].integer == 0, "Use an owned idle verification profile.")
        await store.refresh()
        await store.refreshAgents()
        try #require(store.connected && store.agentCatalogState == .supported)
        let originalSettings = store.settings
        let originalCatalog = store.agentCatalog
        let local = try #require(originalCatalog["providers"].items.first { $0.id == "local" })
        let originalConfig = local["config"]
        do {
            var config = originalConfig
            config["endpoint"] = .string("HTTP://127.0.0.1:11435/v1")
            let savedProvider = await store.perform("/api/settings/providers/local", method: "PUT",
                                                   body: NativeAgentSettingsPolicy.providerUpdate(config: config))
            try #require(savedProvider != nil)
            #expect(store.agentCatalogState == .supported)
            #expect(store.agentCatalog["providers"].items.first { $0.id == "local" }?["config"] == config)

            let codex = JSON.object(["kind": .string("codex"), "provider": .string("local")])
            let staleDraft = NativeAgentSettingsPolicy.defaultsUpdate(settings: originalSettings, selection: codex,
                                                                      version: "1.2.3")
            await store.queueSettings(staleDraft).value
            try #require(store.error == nil)
            let persisted = try await store.api.request("/api/settings")["settings"]
            #expect(persisted["agent"]["kind"].text == "codex")
            #expect(persisted["agent"]["provider"].text == "local")
            #expect(persisted["agent"]["version"].text == "1.2.3")
            #expect(persisted["model"].text == "default")
            #expect(persisted["providers"]["local"] == config)
            #expect(persisted["memory_mb"] == originalSettings["memory_mb"])

            let command = "printf '%s' '{prompt}' '{model}' '{endpoint}'"
            let custom = NativeAgentSettingsPolicy.customValue(name: "Native Settings verification", install: "",
                                                               interactive: command, headless: "", resume: "")
            var reusableDraft = store.settings
            reusableDraft["custom_cli"] = custom
            await store.saveSettings(reusableDraft)
            try #require(store.error == nil)
            let selection = JSON.object(["kind": .string("custom"), "provider": .string("local")])
            let defaults = NativeAgentSettingsPolicy.defaultsUpdate(settings: store.settings, selection: selection,
                                                                    version: "")
            await store.queueSettings(defaults).value
            try #require(store.error == nil)
            let savedCustom = try await store.api.request("/api/settings")["settings"]
            #expect(savedCustom["agent"]["kind"].text == "custom")
            #expect(savedCustom["agent"]["custom"].isNull)
            #expect(savedCustom["agent"]["version"].isNull)
            #expect(savedCustom["custom_cli"]["interactive"].text == command)
            #expect(savedCustom["custom_cli"]["headless"].isNull)
            #expect(savedCustom["custom_cli"]["install"].isNull)
            await store.refreshAgents()
            #expect(store.agentCatalog["providers"].items.map { $0["has_key"] }
                    == originalCatalog["providers"].items.map { $0["has_key"] })
        } catch {
            let failure = error
            do { try await restore(store, settings: originalSettings, config: originalConfig) }
            catch { Issue.record("Could not restore the owned verification profile: \(error.localizedDescription)") }
            throw failure
        }
        try await restore(store, settings: originalSettings, config: originalConfig)
        let restored = try await store.api.request("/api/settings")["settings"]
        #expect(restored["agent"] == originalSettings["agent"])
        #expect(restored["model"] == originalSettings["model"])
        #expect(restored["custom_cli"] == originalSettings["custom_cli"])
        await store.refreshAgents()
        #expect(store.agentCatalog["providers"] == originalCatalog["providers"])
    }

    @MainActor
    private func restore(_ store: NativeStore, settings: JSON, config: JSON) async throws {
        // Provider overrides use dedicated updates; restoring a default leaves an equivalent explicit override.
        _ = try await store.api.request("/api/settings/providers/local", method: "PUT",
                                        body: NativeAgentSettingsPolicy.providerUpdate(config: config))
        _ = try await store.api.request("/api/settings", method: "PUT", body: settings)
    }
}
