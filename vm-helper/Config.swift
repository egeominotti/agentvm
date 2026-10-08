// Configurazione di una VM, letta dal JSON scritto dal server (protocollo §3.5 della spec).
import Foundation
import Virtualization

struct VMConfig: Decodable {
    let disk: URL
    let efivars: URL
    let share: URL
    let console: URL
    let cpus: Int
    let memoryMB: UInt64
    let seedISO: URL?

    enum CodingKeys: String, CodingKey {
        case disk, efivars, share, console, cpus
        case memoryMB = "memory_mb"
        case seedISO = "seed_iso"
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        func url(_ k: CodingKeys) throws -> URL { URL(fileURLWithPath: try c.decode(String.self, forKey: k)) }
        disk = try url(.disk)
        efivars = try url(.efivars)
        share = try url(.share)
        console = try url(.console)
        cpus = try c.decode(Int.self, forKey: .cpus)
        memoryMB = try c.decode(UInt64.self, forKey: .memoryMB)
        seedISO = try c.decodeIfPresent(String.self, forKey: .seedISO).map { URL(fileURLWithPath: $0) }
    }

    static func load(_ path: String) throws -> VMConfig {
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        let config = try JSONDecoder().decode(VMConfig.self, from: data)
        try config.validate()
        return config
    }

    func validate() throws {
        let fm = FileManager.default
        var isDir: ObjCBool = false
        guard fm.fileExists(atPath: disk.path) else { throw ConfigError("disco assente: \(disk.path)") }
        guard fm.fileExists(atPath: share.path, isDirectory: &isDir), isDir.boolValue else {
            throw ConfigError("cartella condivisa assente: \(share.path)")
        }
        if let seed = seedISO, !fm.fileExists(atPath: seed.path) { throw ConfigError("seed ISO assente: \(seed.path)") }
        let cpuRange = VZVirtualMachineConfiguration.minimumAllowedCPUCount...VZVirtualMachineConfiguration.maximumAllowedCPUCount
        guard cpuRange.contains(cpus) else { throw ConfigError("cpus fuori intervallo \(cpuRange): \(cpus)") }
        let mem = memoryMB << 20
        let memRange = VZVirtualMachineConfiguration.minimumAllowedMemorySize...VZVirtualMachineConfiguration.maximumAllowedMemorySize
        guard memRange.contains(mem) else { throw ConfigError("memory_mb fuori intervallo: \(memoryMB)") }
    }
}

struct ConfigError: Error, CustomStringConvertible {
    let description: String
    init(_ d: String) { description = d }
}
