import AppKit
import SwiftUI

@MainActor
extension NativeStore {
    func nativeSetSetting(_ path: [String], _ value: JSON) {
        func replacing(_ source: JSON, _ path: ArraySlice<String>) -> JSON {
            guard let key = path.first else { return value }
            var updated = source.isNull ? .object([:]) : source
            updated[key] = replacing(source[key], path.dropFirst())
            return updated
        }
        let next = replacing(settings, path[...])
        queueSettings(next)
    }

    func nativeSetting(_ path: [String]) -> JSON {
        path.reduce(settings) { $0[$1] }
    }

    func nativeText(_ path: [String], nullable: Bool = false) -> Binding<String> {
        Binding(get: { self.nativeSetting(path).text }, set: { value in
            self.nativeSetSetting(path, nullable && value.trimmingCharacters(in: .whitespaces).isEmpty
                                  ? .null : .string(value))
        })
    }

    func nativeInteger(_ path: [String]) -> Binding<Int> {
        Binding(get: { self.nativeSetting(path).integer }, set: { self.nativeSetSetting(path, .number(Double($0))) })
    }

    func nativeFlag(_ path: [String]) -> Binding<Bool> {
        Binding(get: { self.nativeSetting(path).flag }, set: { self.nativeSetSetting(path, .bool($0)) })
    }
}

struct NativeSettingsNote: View {
    let text: String
    var body: some View {
        Text(text).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
    }
}

struct NativeSettingsError: View {
    let message: String?
    var body: some View {
        if let message {
            Label(message, systemImage: "exclamationmark.triangle")
                .foregroundStyle(.red).textSelection(.enabled)
        }
    }
}

@MainActor
enum NativeFilePanels {
    static func folder() -> URL? {
        let panel = NSOpenPanel()
        panel.title = "Choose a repository folder"
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        return panel.runModal() == .OK ? panel.url : nil
    }

    static func archive() -> URL? {
        let panel = NSOpenPanel()
        panel.title = "Import a snapshot archive"
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.message = "Choose an AgentVM snapshot (.tar.zst, .tar or .tar.gz)."
        return panel.runModal() == .OK ? panel.url : nil
    }

    static func export(named name: String) -> URL? {
        let panel = NSSavePanel()
        panel.title = "Export snapshot"
        panel.nameFieldStringValue = "\(name).tar.zst"
        panel.canCreateDirectories = true
        return panel.runModal() == .OK ? panel.url : nil
    }
}
