//! VM names and the proxy hosts that reach their ports.

use agentvm::domain::hostname;
use agentvm::domain::ids::TaskId;

#[test]
fn every_vm_gets_a_short_stable_dns_name() {
    let id = TaskId::parse("20261008-185855-4f94").unwrap();
    assert_eq!(hostname::vm_name("demo-web", &id), "demo-web-4f94");
    assert_eq!(hostname::vm_name("Fix the parser in src/lib.rs!", &id), "fix-the-parser-in-src-lib-4f94");
    assert_eq!(hostname::vm_name("   ", &id), "vm-4f94");
    assert_eq!(hostname::vm_name("Ünïcode ✨ app", &id), "n-code-app-4f94");
    assert!(hostname::vm_name(&"x".repeat(200), &id).len() <= 40);
}

#[test]
fn proxy_hosts_are_recognised_only_on_this_server() {
    assert_eq!(
        hostname::parse_proxy_host("3000.demo-web-4f94.localhost:7777", 7777),
        Some((3000, "demo-web-4f94".into()))
    );
    assert_eq!(hostname::parse_proxy_host("3000.demo-web-4f94.localhost:7778", 7777), None);
    assert_eq!(hostname::parse_proxy_host("127.0.0.1:7777", 7777), None);
    assert_eq!(hostname::parse_proxy_host("localhost:7777", 7777), None);
    assert_eq!(hostname::parse_proxy_host("demo-web-4f94.localhost:7777", 7777), None);
    assert_eq!(hostname::parse_proxy_host("99999.demo-4f94.localhost:7777", 7777), None);
    assert_eq!(hostname::parse_proxy_host("3000.a.b.localhost:7777", 7777), None);
    assert_eq!(hostname::proxy_url(3000, "demo-web-4f94", 7777), "http://3000.demo-web-4f94.localhost:7777");
}

/// Only the VM's own page speaks for the VM's server ("localhost" inside); any other site keeps
/// its origin, so the server inside (Vite, Jupyter…) can refuse it.
#[test]
fn only_the_vms_own_page_is_given_the_inside_origin() {
    use agentvm::domain::hostname::inside_origin;
    let public = "5173.shop-a1b2.localhost:7777";
    assert_eq!(inside_origin("http://5173.shop-a1b2.localhost:7777", public, 5173), "http://localhost:5173");
    assert_eq!(inside_origin("https://evil.example", public, 5173), "https://evil.example");
    assert_eq!(
        inside_origin("http://8888.other-c3d4.localhost:7777", public, 5173),
        "http://8888.other-c3d4.localhost:7777"
    );
    assert_eq!(inside_origin("null", public, 5173), "null");
}
