// Guest inference HTTP travels over vsock to a private, per-VM host Unix socket.
import Foundation
import Virtualization

final class HostBridge: NSObject, VZVirtioSocketListenerDelegate {
    static let port: UInt32 = 5001
    private let path: String
    private let device: VZVirtioSocketDevice
    private let socketListener = VZVirtioSocketListener()
    private var connections: [ObjectIdentifier: VZVirtioSocketConnection] = [:]

    init(path: String, device: VZVirtioSocketDevice) {
        self.path = path
        self.device = device
        super.init()
    }

    func start() {
        socketListener.delegate = self
        device.setSocketListener(socketListener, forPort: Self.port)
    }

    func listener(_ listener: VZVirtioSocketListener, shouldAcceptNewConnection connection: VZVirtioSocketConnection,
                  from socketDevice: VZVirtioSocketDevice) -> Bool {
        let key = ObjectIdentifier(connection)
        connections[key] = connection
        Thread.detachNewThread { [self] in
            let client = Self.connectUnix(path)
            guard client >= 0 else {
                DispatchQueue.main.async { connection.close(); self.connections[key] = nil }
                return
            }
            let guest = connection.fileDescriptor
            let group = DispatchGroup()
            for (from, to) in [(client, guest), (guest, client)] {
                group.enter()
                Thread.detachNewThread {
                    VsockBridge.pump(from: from, to: to)
                    shutdown(to, SHUT_WR)
                    group.leave()
                }
            }
            group.notify(queue: .main) {
                close(client)
                connection.close()
                self.connections[key] = nil
            }
        }
        return true
    }

    private static func connectUnix(_ path: String) -> Int32 {
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { return -1 }
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8CString)
        guard bytes.count <= MemoryLayout.size(ofValue: address.sun_path) else { close(fd); return -1 }
        withUnsafeMutableBytes(of: &address.sun_path) { raw in
            raw.copyBytes(from: bytes.map { UInt8(bitPattern: $0) })
        }
        let result = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard result == 0 else { close(fd); return -1 }
        return fd
    }
}
