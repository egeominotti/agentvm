import Foundation

struct NativeDiffFile: Identifiable {
    let id: Int
    var path: String
    var added = 0
    var removed = 0
    var lines: [String] = []
}

struct NativeRestoredSessions {
    let active: String
    let extras: [String]
    let mounted: Set<String>
}

enum NativeMachineLogic {
    static let endedStates = Set(["done", "no_changes", "failed", "stopped"])
    static let extraNames = (2...9).map { "shell-\($0)" }

    static func state(_ task: JSON) -> String { task["status"]["state"].text }
    static func ended(_ task: JSON) -> Bool { endedStates.contains(state(task)) }
    static func saveOrCloseBusy(base: String, busy: Set<String>) -> Bool {
        busy.contains("\(base)/save") || busy.contains("\(base)/close")
    }

    static func title(_ task: JSON) -> String {
        let prompt = task["prompt"].text
        if !prompt.isEmpty { return prompt.components(separatedBy: .newlines).first ?? prompt }
        if !task["label"].text.isEmpty { return task["label"].text }
        return URL(fileURLWithPath: task["repo"].text).lastPathComponent
    }

    static func category(_ task: JSON) -> String {
        guard state(task) == "running", !task["interactive"].flag || task["ready"].flag else { return "starting" }
        if !task["interactive"].flag || task["activity"].text == "working" { return "working" }
        return task["activity"].text == "waiting" ? "waiting" : "ready"
    }

    static func status(_ task: JSON) -> String {
        switch state(task) {
        case "queued": return "Queued"
        case "preparing", "booting": return "Starting"
        case "collecting": return "Saving"
        case "done": return "Done, \(task["status"]["commits"].integer) commits"
        case "no_changes": return "No changes"
        case "failed": return "Failed"
        case "stopped": return "Stopped"
        default:
            switch category(task) {
            case "waiting": return "Waiting for you"
            case "working": return "Working"
            case "starting": return "Starting"
            default: return "Ready"
            }
        }
    }

    static func matches(_ task: JSON, query: String) -> Bool {
        let query = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return true }
        return "\(title(task)) \(task["repo"].text) \(task["branch"].text) \(task["id"].text)"
            .localizedCaseInsensitiveContains(query)
    }

    static func sorted(_ tasks: [JSON], newest: Bool) -> [JSON] {
        tasks.sorted {
            if ended($0) != ended($1) { return !ended($0) }
            let key = ended($0) ? "finished_at" : "created_at"
            return newest ? $0[key].number > $1[key].number : $0[key].number < $1[key].number
        }
    }

    static func shells(_ names: [String]) -> [String] { extraNames.filter { names.contains($0) } }
    static func nextShell(_ names: [String]) -> String? { extraNames.first { !names.contains($0) } }
    static func activeSession(_ stored: String?, extras: [String]) -> String {
        guard let stored, (["claude", "shell"] + shells(extras)).contains(stored) else { return "claude" }
        return stored
    }
    static func restoreSessions(stored: String?, extras: [String]) -> NativeRestoredSessions {
        let remembered = shells(extras)
        let active = activeSession(stored, extras: remembered)
        return NativeRestoredSessions(active: active, extras: remembered, mounted: Set(["claude", active]))
    }
    static func closedSessions(name: String, state: NativeRestoredSessions, succeeded: Bool) -> NativeRestoredSessions {
        guard succeeded else { return state }
        var mounted = state.mounted
        mounted.remove(name)
        let active = state.active == name ? "shell" : state.active
        if state.active == name { mounted.insert("shell") }
        return NativeRestoredSessions(active: active, extras: state.extras.filter { $0 != name }, mounted: mounted)
    }
    static func shellLabel(_ name: String) -> String {
        name == "claude" ? "Claude" : name == "shell" ? "Shell" : "Shell \(name.dropFirst(6))"
    }

    static func shellQuote(_ path: String) -> String {
        if path.range(of: #"^[\w@%+=:,./-]+$"#, options: .regularExpression) != nil { return path }
        return "'" + path.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }

    static func diff(_ text: String) -> [NativeDiffFile] {
        var files: [NativeDiffFile] = []
        var inHunk = false
        for line in text.components(separatedBy: "\n") {
            if line.hasPrefix("diff --git ") {
                let path = NativeGitDiffPaths.header(line) ?? line
                files.append(NativeDiffFile(id: files.count, path: path))
                inHunk = false
            } else if !files.isEmpty {
                if !inHunk, let path = NativeGitDiffPaths.metadata(line) { files[files.count - 1].path = path }
                let metadata = ["index ", "--- ", "+++ ", "new file mode", "deleted file mode", "similarity ", "rename "]
                if !inHunk && metadata.contains(where: line.hasPrefix) { continue }
                if line.hasPrefix("@@") { inHunk = true }
                let last = files.count - 1
                if line.hasPrefix("+") { files[last].added += 1 }
                if line.hasPrefix("-") { files[last].removed += 1 }
                files[last].lines.append(line)
            }
        }
        return files
    }
}
