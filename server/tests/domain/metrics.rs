//! Telemetry from the guest: VM metrics and Claude's usage.

#[test]
fn vm_metrics_parse_guest_json() {
    use agentvm::domain::metrics::VmMetrics;
    let m: VmMetrics = serde_json::from_str(
        r#"{"uptime_s":12,"cpus":4,"cpu_pct":37.5,"load1":0.8,"mem_used_mb":900,"mem_total_mb":3922,
            "disk_used_mb":3100,"disk_total_mb":19800,"net_rx_bps":1200,"net_tx_bps":300,"procs":91,
            "top":[{"name":"cargo","cpu_pct":180.2,"mem_mb":410}]}"#,
    )
    .unwrap();
    assert_eq!(m.top[0].name, "cargo");
    assert_eq!(m.cpus, 4);
}

#[test]
fn agent_usage_parses_the_status_line_copy() {
    use agentvm::domain::usage::AgentUsage;
    let u: AgentUsage = serde_json::from_str(
        r#"{"cost_usd":0.1234,"input_tokens":15234,"output_tokens":2100,"lines_added":12,"lines_removed":3,"context_pct":7,"model":"Sonnet 5.5"}"#,
    )
    .unwrap();
    assert_eq!(u.input_tokens, 15234);
    assert!((u.cost_usd - 0.1234).abs() < 1e-9);
    let empty: AgentUsage = serde_json::from_str("{}").unwrap();
    assert_eq!(empty.output_tokens, 0);
}
