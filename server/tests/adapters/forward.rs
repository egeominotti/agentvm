//! A VM's TCP service reached from the Mac.

use agentvm::adapters::forward::PortForward;

/// A VM must never take a port of this Mac that one of its own services uses or will use
/// (5432, 6379…): local clients would talk to the VM. The forward gets a port of its own.
#[tokio::test]
async fn a_vm_service_never_takes_the_same_port_on_the_mac() {
    // Below the system's automatic range (49152 and up), so the port the forward is given can
    // never be this one by chance; free when the test starts.
    let port = (20_000..30_000).find(|p| std::net::TcpListener::bind(("127.0.0.1", *p)).is_ok()).expect("a free port");
    let fwd = PortForward::start(port, std::path::PathBuf::from("/nonexistent.sock")).unwrap();
    assert_ne!(fwd.host_port, port, "the forward took the VM's port number on the Mac");
}
