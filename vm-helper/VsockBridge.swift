// Bridge from a Unix socket (on the Mac) → guest vsock port: each accepted connection becomes a
// channel to the PTY server in the VM.
import Foundation
import Virtualization

final class VsockBridge {
    static let ptyPort: UInt32 = 5000

    private let path: String
    private let port: UInt32
    private let device: VZVirtioSocketDevice
    /// The vsock connections must be kept alive while the channel is open.
    private var connections: [ObjectIdentifier: VZVirtioSocketConnection] = [:]

    init(path: String, port: UInt32, device: VZVirtioSocketDevice) {
        self.path = path
        self.port = port
        self.device = device
    }

    func start() throws {
        unlink(path)
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw ConfigError("socket(): \(errno)") }
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8CString)
        guard bytes.count <= MemoryLayout.size(ofValue: addr.sun_path) else { throw ConfigError("path too long: \(path)") }
        withUnsafeMutableBytes(of: &addr.sun_path) { raw in
            raw.copyBytes(from: bytes.map { UInt8(bitPattern: $0) })
        }
        let bound = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
        }
        guard bound == 0, listen(fd, 128) == 0 else { throw ConfigError("bind/listen \(path): \(errno)") }
        chmod(path, 0o600)
        Thread.detachNewThread { [self] in
            while true {
                let client = accept(fd, nil, nil)
                // Out of descriptors (EMFILE) or similar: back off instead of spinning a core.
                if client < 0 { usleep(100_000); continue }
                DispatchQueue.main.async { self.connect(client) }
            }
        }
    }

    /// Must be called on the main queue (the VM's queue). A refused connection (the guest busy
    /// accepting many at once) is tried again a few times before the channel is given up.
    private func connect(_ client: Int32, attempt: Int = 1) {
        device.connect(toPort: port) { [self] result in
            switch result {
            // Up to ~3.6 s in all: a guest busy booting or accepting many terminals catches up.
            case .failure where attempt < 9:
                DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(100 * attempt)) {
                    self.connect(client, attempt: attempt + 1)
                }
            case .failure:
                close(client)
            case .success(let conn):
                let key = ObjectIdentifier(conn)
                connections[key] = conn
                let guest = conn.fileDescriptor
                let group = DispatchGroup()
                for (from, to) in [(client, guest), (guest, client)] {
                    group.enter()
                    Thread.detachNewThread {
                        Self.pump(from: from, to: to)
                        shutdown(to, SHUT_WR)
                        group.leave()
                    }
                }
                group.notify(queue: .main) { [self] in
                    close(client)
                    conn.close()
                    connections[key] = nil
                }
            }
        }
    }

    private static func pump(from: Int32, to: Int32) {
        var buf = [UInt8](repeating: 0, count: 65536)
        while true {
            let n = read(from, &buf, buf.count)
            if n <= 0 { return }
            var off = 0
            while off < n {
                let w = buf.withUnsafeBytes { write(to, $0.baseAddress! + off, n - off) }
                if w <= 0 { return }
                off += w
            }
        }
    }
}
