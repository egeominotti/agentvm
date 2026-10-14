import Foundation

extension JSON {
    var id: String { self["id"].text }
    var state: String { self["status"]["state"].text }
    var isEnded: Bool { ["done", "no_changes", "failed", "stopped"].contains(state) }
    var isInteractive: Bool { self["interactive"].flag }
    var isWaiting: Bool { state == "running" && self["activity"].text == "waiting" }
    var shortID: String { String(id.suffix(4)) }
    var repoName: String {
        self["repo"].text.split(separator: "/").last.map(String.init) ?? self["repo"].text
    }
    var title: String {
        let prompt = self["prompt"].text
        if !prompt.isEmpty { return prompt.components(separatedBy: .newlines).first ?? prompt }
        let label = self["label"].text
        return label.isEmpty ? repoName : label
    }
    var stateLabel: String {
        switch state {
        case "queued": return "Queued"
        case "preparing", "booting": return "Starting"
        case "collecting": return "Saving"
        case "done":
            let commits = self["status"]["commits"].integer
            return "Done, \(commits) \(commits == 1 ? "commit" : "commits")"
        case "no_changes": return "No changes"
        case "failed": return "Failed"
        case "stopped": return "Stopped"
        case "running":
            if isInteractive && !self["ready"].flag { return "Starting" }
            if !isInteractive || self["activity"].text == "working" { return "Working" }
            return isWaiting ? "Waiting for you" : "Ready"
        default: return "Unknown"
        }
    }
}
