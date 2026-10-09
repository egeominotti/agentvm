// Translates VMConfig into a Virtualization.framework configuration.
import Foundation
import Virtualization

enum MachineFactory {
    /// virtiofs tag mounted by the guest at /mnt/job.
    static let shareTag = "job"

    static func make(_ c: VMConfig) throws -> VZVirtualMachineConfiguration {
        let cfg = VZVirtualMachineConfiguration()
        cfg.platform = VZGenericPlatformConfiguration()
        cfg.bootLoader = try bootLoader(c)
        cfg.cpuCount = c.cpus
        cfg.memorySize = c.memoryMB << 20
        cfg.storageDevices = try storage(c)
        cfg.networkDevices = [natNetwork()]
        cfg.serialPorts = [try console(c.console)]
        cfg.directorySharingDevices = [share(c.share)]
        cfg.entropyDevices = [VZVirtioEntropyDeviceConfiguration()]
        cfg.memoryBalloonDevices = [VZVirtioTraditionalMemoryBalloonDeviceConfiguration()]
        cfg.socketDevices = [VZVirtioSocketDeviceConfiguration()]
        try cfg.validate()
        return cfg
    }

    /// Straight into the kernel when the server gives one (no firmware, no GRUB: ~0.7 s sooner),
    /// else through EFI. The EFI variables are there either way: a snapshot keeps them, and a VM
    /// restored from it boots through EFI from its own disk, whose kernel may be another.
    private static func bootLoader(_ c: VMConfig) throws -> VZBootLoader {
        let efi = try efiBootLoader(efivars: c.efivars)
        guard let kernel = c.kernel else { return efi }
        let linux = VZLinuxBootLoader(kernelURL: kernel)
        linux.initialRamdiskURL = c.initrd
        linux.commandLine = c.cmdline ?? ""
        return linux
    }

    private static func efiBootLoader(efivars: URL) throws -> VZEFIBootLoader {
        let boot = VZEFIBootLoader()
        boot.variableStore = FileManager.default.fileExists(atPath: efivars.path)
            ? VZEFIVariableStore(url: efivars)
            : try VZEFIVariableStore(creatingVariableStoreAt: efivars)
        return boot
    }

    private static func storage(_ c: VMConfig) throws -> [VZStorageDeviceConfiguration] {
        var devices: [VZStorageDeviceConfiguration] = [
            // A guest flush becomes a plain fsync, not a full device flush (F_FULLFSYNC, ~90x
            // slower): apt, git and databases flush constantly. Snapshots still flush the
            // guest (sync) and the host file (fsync) before cloning the disk.
            VZVirtioBlockDeviceConfiguration(attachment: try VZDiskImageStorageDeviceAttachment(
                url: c.disk, readOnly: false, cachingMode: .automatic, synchronizationMode: .fsync)),
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
