import Foundation

enum NativeSnapshotKind: String, CaseIterable, Identifiable, Sendable {
    case all, manual, automatic, interrupted
    var id: String { rawValue }
    var title: String { rawValue.capitalized }
}

struct NativeSnapshotGroup: Identifiable, Sendable {
    let id: String
    let title: String
    let repo: String
    let latest: Double
    let snapshots: [JSON]
}

enum NativeSnapshotPolicy {
    static func kind(_ snapshot: JSON) -> NativeSnapshotKind {
        if snapshot["name"].text.hasPrefix("Interrupted: ") { return .interrupted }
        return snapshot["auto"].flag ? .automatic : .manual
    }

    static func filtered(_ snapshots: [JSON], query: String, kind wanted: NativeSnapshotKind) -> [JSON] {
        let query = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return snapshots.filter {
            (wanted == .all || kind($0) == wanted) &&
            (query.isEmpty || ($0["name"].text + " " + $0["repo"].text).lowercased().contains(query))
        }
    }

    static func selected(_ shown: [JSON], ids: Set<String>) -> [String] {
        shown.map { $0["id"].text }.filter(ids.contains)
    }

    static func groups(_ snapshots: [JSON]) -> [NativeSnapshotGroup] {
        Dictionary(grouping: snapshots, by: { $0["source_task"].text }).map { task, values in
            let newest = values.sorted { $0["created_at"].number > $1["created_at"].number }
            let interrupted = newest.filter { kind($0) == .interrupted }
            let automatic = newest.filter { kind($0) == .automatic }
            let manual = newest.filter { kind($0) == .manual }
            let generated = interrupted.first ?? automatic.first
            let repo = newest.first?["repo"].text ?? ""
            let title = generated.map { baseTitle($0["name"].text) }
                ?? "\(URL(fileURLWithPath: repo).lastPathComponent) #\(task.suffix(4))"
            return NativeSnapshotGroup(id: task, title: title, repo: repo,
                                       latest: newest.first?["created_at"].number ?? 0,
                                       snapshots: interrupted + manual + automatic)
        }.sorted { $0.latest == $1.latest ? $0.id < $1.id : $0.latest > $1.latest }
    }

    static func label(_ snapshot: JSON, title: String) -> String {
        let name = snapshot["name"].text
        if kind(snapshot) == .interrupted { return "Interrupted" }
        if name.range(of: #", auto \d{1,2}:\d{2}$"#, options: .regularExpression) != nil { return "Automatic" }
        if name.range(of: #", before close \d{1,2}:\d{2}$"#, options: .regularExpression) != nil { return "Before closing" }
        return name == title ? "Snapshot" : name
    }

    private static func baseTitle(_ name: String) -> String {
        name.replacingOccurrences(of: "^Interrupted: ", with: "", options: .regularExpression)
            .replacingOccurrences(of: #", (auto|before close) \d{1,2}:\d{2}$"#, with: "", options: .regularExpression)
    }
}
