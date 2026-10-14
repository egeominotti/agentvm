import Foundation

struct NativeSettingSection: Identifiable, Sendable {
    let id: String
    let title: String
    let group: String
    let symbol: String
    let keywords: String
}

enum NativeSettingsPolicy {
    static let models = [
        ("default", "Account default"), ("opus", "Opus"), ("sonnet", "Sonnet"), ("haiku", "Haiku"),
        ("fable", "Fable"), ("opusplan", "Opus plans, Sonnet builds"),
        ("opus[1m]", "Opus, 1M context"), ("sonnet[1m]", "Sonnet, 1M context")
    ]
    static let intervals = [0, 1, 5, 10, 15, 30, 60, 120, 240]
    static let catalog: [NativeSettingSection] = [
        .init(id: "general", title: "General", group: "Workspace", symbol: "gearshape",
              keywords: "model default repository time limit timeout agent"),
        .init(id: "resources", title: "Resources", group: "Workspace", symbol: "cpu",
              keywords: "vms cpu cores memory ram concurrency"),
        .init(id: "preferences", title: "Preferences", group: "Workspace", symbol: "sun.max",
              keywords: "theme light dark appearance notifications"),
        .init(id: "account", title: "Claude account", group: "Accounts & access", symbol: "person.crop.circle",
              keywords: "token subscription keychain claude"),
        .init(id: "git", title: "Git access", group: "Accounts & access", symbol: "arrow.triangle.branch",
              keywords: "github gitlab token private repositories"),
        .init(id: "tailscale", title: "Tailscale", group: "Accounts & access", symbol: "network",
              keywords: "tailnet vpn ssh auth key network"),
        .init(id: "snapshots", title: "Snapshots", group: "Data", symbol: "camera",
              keywords: "automatic schedule interval keep close"),
        .init(id: "backups", title: "Backups", group: "Data", symbol: "externaldrive.badge.icloud",
              keywords: "s3 r2 hetzner backblaze minio rustfs bucket"),
        .init(id: "storage", title: "Storage", group: "Data", symbol: "internaldrive",
              keywords: "disk space logs cleanup delete"),
        .init(id: "image", title: "VM image", group: "System", symbol: "opticaldisc",
              keywords: "golden debian rebuild claude code version")
    ]

    static func sections(matching query: String) -> [NativeSettingSection] {
        let words = query.lowercased().split(whereSeparator: \.isWhitespace)
        return catalog.filter { section in
            let text = (section.title + " " + section.keywords).lowercased()
            return words.allSatisfy { text.contains($0) }
        }
    }

    static func memoryChoices(ram: Int, current: Int) -> [Int] {
        let maximum = max(1024, ram - 8192)
        let usual = [1024, 2048, 4096, 6144, 8192, 12288, 16384].filter { $0 <= maximum }
        return Set(usual + (current > 0 ? [current] : [])).sorted()
    }

    static func intervalLabel(_ minutes: Int) -> String {
        if minutes == 0 { return "Never" }
        if minutes < 60 { return "Every \(minutes) minute\(minutes == 1 ? "" : "s")" }
        return "Every \(minutes / 60) hour\(minutes == 60 ? "" : "s")"
    }

    static func size(_ mb: Int) -> String {
        mb < 1024 ? "\(mb) MB" : String(format: "%.1f GB", Double(mb) / 1024)
    }

    static let providers = [
        ("aws", "AWS S3", "https://s3.eu-central-1.amazonaws.com", "eu-central-1", false),
        ("r2", "Cloudflare R2", "https://<account-id>.r2.cloudflarestorage.com", "auto", true),
        ("hetzner", "Hetzner", "https://fsn1.your-objectstorage.com", "fsn1", false),
        ("b2", "Backblaze B2", "https://s3.eu-central-003.backblazeb2.com", "eu-central-003", false),
        ("local", "Local (RustFS / MinIO)", "http://127.0.0.1:9100", "us-east-1", true)
    ]

    static func preset(_ id: String, applyingTo value: JSON) -> JSON {
        guard let preset = providers.first(where: { $0.0 == id }) else { return value }
        var config = value.isNull ? .object(["prefix": .string("agentvm")]) : value
        config["endpoint"] = .string(preset.2)
        config["region"] = .string(preset.3)
        config["path_style"] = .bool(preset.4)
        if id == "local" {
            if config["bucket"].text.isEmpty { config["bucket"] = .string("agentvm-backups") }
            if config["access_key"].text.isEmpty { config["access_key"] = .string("agentvm") }
        }
        return config
    }
}
