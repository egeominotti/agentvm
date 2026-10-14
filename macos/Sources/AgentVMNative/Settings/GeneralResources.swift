import SwiftUI

struct NativeGeneralPane: View {
    @ObservedObject var store: NativeStore

    var body: some View {
        Form {
            Section("Agent") {
                if !store.agentCatalog.isNull {
                    NativeAgentDefaultsFields(store: store)
                } else if store.agentCatalogState == .legacy {
                    LabeledContent("Default coding CLI", value: "Claude Code")
                    NativeAgentCatalogStatus(store: store)
                } else {
                    NativeAgentCatalogStatus(store: store)
                }
            }
            Section("Workspace defaults") {
                TextField("Default repository", text: store.nativeText(["default_repo"], nullable: true))
                Button("Choose repository folder…") {
                    if let folder = NativeFilePanels.folder() {
                        store.nativeSetSetting(["default_repo"], .string(folder.path))
                    }
                }
                NativeSettingsNote(text: "A folder or git link suggested for new VMs. Each launch can choose otherwise.")
                Stepper("Automatic task time limit: \(max(1, store.settings["timeout_s"].integer / 60)) minutes",
                        value: Binding(get: { max(1, store.settings["timeout_s"].integer / 60) },
                                       set: { store.nativeSetSetting(["timeout_s"], .number(Double($0 * 60))) }),
                        in: 1...1440)
                NativeSettingsNote(text: "Repository and timeout changes save automatically. Tasks without a terminal stop at this limit. Interactive terminals never time out.")
            }
        }
    }
}

struct NativeResourcesPane: View {
    @ObservedObject var store: NativeStore
    private var ram: Int { store.limits["ram_mb"].integer }
    private var memory: Int { max(1024, store.settings["memory_mb"].integer) }
    private var capacity: Int { max(1, (ram - 8192) / memory) }

    var body: some View {
        Form {
            Section("This Mac") {
                LabeledContent("CPU cores", value: "\(store.limits["cpus"].integer)")
                LabeledContent("Memory", value: NativeSettingsPolicy.size(ram))
                NativeSettingsNote(text: "8 GB stays available for macOS and your applications.")
            }
            Section("Each new VM") {
                Stepper("vCPUs: \(store.settings["cpus"].integer)", value: store.nativeInteger(["cpus"]),
                        in: 1...max(1, store.limits["cpus"].integer))
                Picker("Memory", selection: store.nativeInteger(["memory_mb"])) {
                    ForEach(NativeSettingsPolicy.memoryChoices(ram: ram, current: memory), id: \.self) {
                        Text(NativeSettingsPolicy.size($0)).tag($0)
                    }
                }
                NativeSettingsNote(text: "Running VMs keep the resources they started with.")
            }
            Section("Capacity") {
                Stepper("VMs at the same time: \(store.settings["max_vms"].integer)",
                        value: store.nativeInteger(["max_vms"]), in: 1...64)
                LabeledContent("Memory at full load",
                               value: NativeSettingsPolicy.size(store.settings["max_vms"].integer * memory))
                NativeSettingsNote(text: "Up to \(capacity) VMs of \(NativeSettingsPolicy.size(memory)) fit without swapping.")
                if store.settings["max_vms"].integer > capacity {
                    Label("Full load exceeds the memory budget. Launches wait for available memory.",
                          systemImage: "exclamationmark.triangle").foregroundStyle(.orange)
                }
            }
        }
    }
}
