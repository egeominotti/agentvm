import AppKit
import Foundation

enum MachineFormat {
    static func memory(_ mb: Double) -> String {
        mb >= 1024 ? String(format: mb >= 10240 ? "%.0f GB" : "%.1f GB", mb / 1024) : "\(Int(mb.rounded())) MB"
    }

    static func money(_ value: Double) -> String {
        if value == 0 { return "$0" }
        if value < 0.01 { return "<$0.01" }
        return String(format: value < 1 ? "$%.3f" : value < 100 ? "$%.2f" : "$%.0f", value)
    }

    static func tokens(_ value: Double) -> String {
        if value >= 1_000_000 { return String(format: "%.1fM", value / 1_000_000) }
        if value >= 1000 { return String(format: value >= 10_000 ? "%.0fk" : "%.1fk", value / 1000) }
        return "\(Int(value))"
    }

    static func rate(_ value: Double) -> String {
        if value < 1024 { return "\(Int(value.rounded())) B/s" }
        if value < 1024 * 1024 { return String(format: "%.0f KB/s", value / 1024) }
        return String(format: "%.1f MB/s", value / 1024 / 1024)
    }

    static func duration(_ seconds: Double) -> String {
        let value = max(0, Int(seconds.rounded()))
        if value < 60 { return "\(value)s" }
        if value < 3600 { return "\(value / 60)m \(value % 60)s" }
        return "\(value / 3600)h \(value / 60 % 60)m"
    }

    static func age(_ task: JSON) -> String {
        let end = task["finished_at"].isNull ? Date().timeIntervalSince1970 : task["finished_at"].number
        return duration(end - task["created_at"].number)
    }

    static func date(_ epoch: Double) -> String {
        Date(timeIntervalSince1970: epoch).formatted(date: .abbreviated, time: .shortened)
    }

    @MainActor static func copy(_ text: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }
}
