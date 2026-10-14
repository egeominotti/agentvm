import Foundation

enum NativeInspectorLogic {
    static func percentQuery(_ text: String) -> String {
        let allowed = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~")
        return text.addingPercentEncoding(withAllowedCharacters: allowed) ?? ""
    }

    static func cursorQuery(_ cursor: JSON) throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        return percentQuery(String(decoding: try encoder.encode(cursor), as: UTF8.self))
    }

    static func timestamp(_ text: String) -> Date? {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let value = formatter.date(from: text) { return value }
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.date(from: text)
    }

    static func turns(_ entries: [JSON]) -> [JSON] {
        var seen = Set<String>()
        var result: [JSON] = []
        for entry in entries {
            let id = entry["message_id"].text
            guard !id.isEmpty, !seen.contains(id), !entry["usage"].isNull,
                  let date = timestamp(entry["at"].text) else { continue }
            seen.insert(id)
            var usage = entry["usage"]
            usage["at"] = .number(date.timeIntervalSince1970)
            result.append(usage)
        }
        return result.sorted { $0["at"].number < $1["at"].number }
    }

    static func report(id: String, diagnostics: JSON) -> String {
        var lines = ["agentvm diagnostics — machine \(id)", "", "Why: \(diagnostics["summary"].text)"]
        if !diagnostics["hint"].text.isEmpty { lines.append("What to do: \(diagnostics["hint"].text)") }
        let formatter = ISO8601DateFormatter()
        lines += ["", "Timeline:"]
        for step in diagnostics["timeline"].items {
            lines.append("  \(formatter.string(from: Date(timeIntervalSince1970: step["at"].number)))  \(step["state"].text)")
        }
        for log in diagnostics["logs"].items {
            lines += ["", "--- \(log["name"].text) (\(log["file"].text)) ---", log["tail"].text]
        }
        if !diagnostics["server_log"].items.isEmpty {
            lines += ["", "--- Server log ---"] + diagnostics["server_log"].items.map(\.text)
        }
        return lines.joined(separator: "\n")
    }

    static func firstLog(_ diagnostics: JSON) -> String? {
        let hint = diagnostics["hint"].text.lowercased()
        let wanted = hint.contains("setup.sh") ? "share/setup.log"
            : hint.contains("console") ? "console.log"
            : hint.contains("claude's errors") ? "share/claude.err" : "share/job.log"
        let logs = diagnostics["logs"].items
        return logs.first(where: { $0["file"].text == wanted })?["file"].text ?? logs.first?["file"].text
    }
}
