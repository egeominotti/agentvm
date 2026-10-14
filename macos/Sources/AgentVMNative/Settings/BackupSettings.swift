import SwiftUI

struct NativeBackupSettingsPane: View {
    @ObservedObject var store: NativeStore
    @State private var config: JSON = .object(["endpoint": .string(""), "region": .string(""),
                                              "bucket": .string(""), "prefix": .string("agentvm"),
                                              "access_key": .string(""), "path_style": .bool(false)])
    @State private var provider = "custom"
    @State private var secret = ""
    @State private var configured = false
    @State private var savedSecret = false
    @State private var saving = false
    @State private var checking = false
    @State private var verified = false
    @State private var connectionError: String?
    @State private var savedConfig: JSON = .null

    private func field(_ key: String) -> Binding<String> {
        Binding(get: { config[key].text }, set: { config[key] = .string($0); verified = false; connectionError = nil })
    }

    var body: some View {
        Form {
            Section("S3 storage") {
                LabeledContent("Status", value: NativeS3Presentation.status(configured: configured,
                               checking: saving || checking, verified: verified, failure: connectionError))
                Picker("Provider", selection: Binding(get: { provider }, set: { next in
                    provider = next
                    config = NativeSettingsPolicy.preset(next, applyingTo: config)
                    verified = false
                    connectionError = nil
                })) {
                    Text("Custom").tag("custom")
                    ForEach(NativeSettingsPolicy.providers, id: \.0) { preset in Text(preset.1).tag(preset.0) }
                }
                TextField("Endpoint", text: field("endpoint"))
                TextField("Region", text: field("region"))
                TextField("Bucket", text: field("bucket"))
                TextField("Folder in the bucket", text: field("prefix"))
                TextField("Access key", text: field("access_key"))
                SecureField(savedSecret ? "Secret key (empty keeps saved key)" : "Secret key", text: $secret)
                    .onChange(of: secret) { _, value in if !value.isEmpty { verified = false; connectionError = nil } }
                Toggle("Path-style addresses", isOn: Binding(get: { config["path_style"].flag },
                                                            set: { config["path_style"] = .bool($0); verified = false; connectionError = nil }))
                NativeSettingsNote(text: "Path-style uses endpoint/bucket/key, required by MinIO and RustFS. Secret keys stay in the macOS Keychain.")
                if provider == "r2" {
                    NativeSettingsNote(text: "Replace <account-id> in the endpoint. Cloudflare R2 uses region auto.")
                }
                ViewThatFits(in: .horizontal) {
                    HStack { connectionButtons }.fixedSize(horizontal: true, vertical: false)
                    VStack(alignment: .leading, spacing: 8) { connectionButtons }
                }
                if config["endpoint"].text == "http://127.0.0.1:9100" {
                    NativeSettingsNote(text: "Local RustFS must keep running on this Mac for backups to be available.")
                    Link("Open the local S3 console", destination: URL(string: "http://127.0.0.1:9101")!)
                }
                NativeSettingsNote(text: "Settings are saved only after the bucket accepts an upload and download.")
                NativeSettingsError(message: connectionError)
            }
        }.task { await load() }.onDisappear { secret = "" }
    }

    @ViewBuilder private var connectionButtons: some View {
        Button(saving ? "Testing connection…" : "Test and save") { Task { await save() } }
            .buttonStyle(.borderedProminent).disabled(saving || checking)
        Button("Test saved connection") { Task { await testSaved() } }
            .disabled(!configured || saving || checking || config != savedConfig || !secret.isEmpty)
    }

    private func testSaved() async {
        checking = true
        defer { checking = false }
        verified = await store.testS3Connection()
        connectionError = verified ? nil : store.error
    }

    private func load() async {
        do {
            let stored = try await store.api.request("/api/settings/s3")
            configured = !stored["config"].isNull
            savedSecret = stored["secret_saved"].flag
            savedConfig = stored["config"]
            if configured {
                config = stored["config"]
                provider = NativeSettingsPolicy.providers.first { $0.2 == config["endpoint"].text }?.0 ?? "custom"
            } else {
                provider = "local"
                config = NativeSettingsPolicy.preset("local", applyingTo: config)
            }
        } catch is CancellationError {
        } catch { if !Task.isCancelled { store.error = error.localizedDescription } }
    }

    private func save() async {
        saving = true
        defer { saving = false }
        let value = secret.trimmingCharacters(in: .whitespacesAndNewlines)
        secret = ""
        let body: JSON = .object(["config": config, "secret": value.isEmpty ? .null : .string(value)])
        if await store.perform("/api/settings/s3", method: "PUT", body: body,
                               success: "Connected: the S3 bucket accepts uploads") != nil {
            verified = true
            connectionError = nil
            await load()
            await store.refreshSnapshots()
        } else {
            verified = false
            connectionError = NativeS3Presentation.message(raw: store.error ?? "The S3 connection could not be checked.",
                                                          endpoint: config["endpoint"].text)
        }
    }
}
