import AppKit
import SwiftUI

struct NativeMachineConnectivity: View {
    @ObservedObject var store: NativeStore
    let task: JSON
    @State private var keyLoaded = false
    @State private var keySaved = false
    @State private var authKey = ""
    @State private var joining = false
    private var base: String { "/api/tasks/\(task["id"].text)/tailscale" }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if !task["ports"].items.isEmpty {
                HStack {
                    Text("Open on this Mac").font(.caption).foregroundStyle(.secondary)
                    ForEach(task["ports"].items, id: \.["port"].integer) { port in
                        let name = port["name"].text.isEmpty ? "service" : port["name"].text
                        if port["kind"].text == "http", !port["url"].text.isEmpty, let url = URL(string: port["url"].text) {
                            Link("\(name) · :\(port["port"].integer)", destination: url).help(url.absoluteString)
                        } else if !port["host_port"].isNull {
                            Button("\(name) · localhost:\(port["host_port"].integer)") {
                                MachineFormat.copy("localhost:\(port["host_port"].integer)")
                                store.notice = "Copied the service address"
                            }.help("TCP service; click to copy its address")
                        }
                    }
                }
            }
            if task["interactive"].flag && NativeMachineLogic.state(task) == "running" && task["ready"].flag {
                HStack {
                    Label("Tailscale", systemImage: "network").font(.caption).foregroundStyle(.secondary)
                    if !task["tailscale"].flag {
                        if keySaved {
                            Text("Reach this VM from your other devices.").font(.caption).foregroundStyle(.secondary)
                            Button("Join tailnet") { Task { _ = await store.perform(base) } }
                        } else if keyLoaded {
                            SecureField("Tailscale auth key", text: $authKey).frame(maxWidth: 250)
                            Button(joining ? "Joining…" : "Save and join") { Task { await saveAndJoin() } }
                                .disabled(authKey.isEmpty || joining)
                        } else { ProgressView().controlSize(.small) }
                    } else if !task["tailnet"]["name"].text.isEmpty {
                        let name = task["tailnet"]["name"].text
                        Button(name) { MachineFormat.copy(name) }.buttonStyle(.link)
                        ForEach(task["tailnet"]["ips"].items.map(\.text), id: \.self) { ip in
                            Button(ip) { MachineFormat.copy(ip) }.buttonStyle(.link)
                        }
                        Button("Copy SSH command") {
                            MachineFormat.copy("ssh root@\(name.components(separatedBy: ".").first ?? name)")
                        }
                        Button("Leave") { Task { _ = await store.perform(base, method: "DELETE") } }
                    } else if !task["tailnet"]["error"].text.isEmpty {
                        Text(task["tailnet"]["error"].text).font(.caption).foregroundStyle(.red).lineLimit(2)
                        Button("Try again") { Task { _ = await store.perform(base) } }
                        Button("Leave") { Task { _ = await store.perform(base, method: "DELETE") } }
                    } else {
                        Text("Joining your tailnet…").font(.caption).foregroundStyle(.secondary)
                        Button("Cancel") { Task { _ = await store.perform(base, method: "DELETE") } }
                    }
                    Spacer()
                }.disabled(store.busy.contains(base))
            }
        }
        .padding(.horizontal, 14).padding(.vertical, 6)
        .task(id: task["id"].text) {
            do {
                let result = try await store.api.request("/api/settings/tailscale")
                guard !Task.isCancelled else { return }
                keySaved = result["key_saved"].flag
                keyLoaded = true
            } catch is CancellationError { }
            catch { if !Task.isCancelled { store.error = error.localizedDescription; keyLoaded = true } }
        }
    }

    private func saveAndJoin() async {
        joining = true
        defer { joining = false }
        guard await store.perform("/api/settings/tailscale", method: "PUT", body: .object(["key": .string(authKey)])) != nil else { return }
        authKey = ""
        keySaved = true
        _ = await store.perform(base)
    }
}
