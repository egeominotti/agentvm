// Traduce VMConfig in una configurazione Virtualization.framework.
import Foundation
import Virtualization

enum MachineFactory {
    /// Tag virtiofs montato dal guest in /mnt/job.
    static let shareTag = "job"

    static func make(_ c: VMConfig) throws -> VZVirtualMachineConfiguration {
        let cfg = VZVirtualMachineConfiguration()
        cfg.platform = VZGenericPlatformConfiguration()
        cfg.bootLoader = try bootLoader(efivars: c.efivars)
        cfg.cpuCount = c.cpus
        cfg.memorySize = c.memoryMB << 20
        cfg.storageDevices = try storage(c)
        cfg.networkDevices = [natNetwork()]
        cfg.serialPorts = [try console(c.console)]
        cfg.directorySharingDevices = [share(c.share)]
        cfg.entropyDevices = [VZVirtioEntropyDeviceConfiguration()]
        cfg.memoryBalloonDevices = [VZVirtioTraditionalMemoryBalloonDeviceConfiguration()]
        try cfg.validate()
        return cfg
    }

    private static func bootLoader(efivars: URL) throws -> VZEFIBootLoader {
        let boot = VZEFIBootLoader()
        boot.variableStore = FileManager.default.fileExists(atPath: efivars.path)
            ? VZEFIVariableStore(url: efivars)
            : try VZEFIVariableStore(creatingVariableStoreAt: efivars)
        return boot
    }

    private static func storage(_ c: VMConfig) throws -> [VZStorageDeviceConfiguration] {
        var devices: [VZStorageDeviceConfiguration] = [
            VZVirtioBlockDeviceConfiguration(attachment: try VZDiskImageStorageDeviceAttachment(url: c.disk, readOnly: false)),
        ]
        if let seed = c.seedISO {
            devices.append(VZVirtioBlockDeviceConfiguration(
                attachment: try VZDiskImageStorageDeviceAttachment(url: seed, readOnly: true)))
        }
        return devices
    }

    private static func natNetwork() -> VZVirtioNetworkDeviceConfiguration {
        let net = VZVirtioNetworkDeviceConfiguration()
        net.attachment = VZNATNetworkDeviceAttachment()
        return net
    }

    private static func console(_ url: URL) throws -> VZVirtioConsoleDeviceSerialPortConfiguration {
        FileManager.default.createFile(atPath: url.path, contents: nil)
        let serial = VZVirtioConsoleDeviceSerialPortConfiguration()
        serial.attachment = VZFileHandleSerialPortAttachment(
            fileHandleForReading: nil, fileHandleForWriting: try FileHandle(forWritingTo: url))
        return serial
    }

    private static func share(_ url: URL) -> VZVirtioFileSystemDeviceConfiguration {
        let fs = VZVirtioFileSystemDeviceConfiguration(tag: shareTag)
        fs.share = VZSingleDirectoryShare(directory: VZSharedDirectory(url: url, readOnly: false))
        return fs
    }
}
