//! Claude's usage over time, kept next to the job for the VM's whole life.

use agentvm::app::history::UsageRecorder;
use agentvm::domain::usage::{AgentUsage, UsageSample};

fn usage(cost: f64, out: u64) -> AgentUsage {
    AgentUsage { cost_usd: cost, output_tokens: out, ..Default::default() }
}

#[test]
fn each_change_of_usage_is_one_sample_and_a_restart_adds_no_duplicate() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("usage.jsonl");
    let mut rec = UsageRecorder::new(file.clone());
    rec.record(&usage(0.10, 100));
    rec.record(&usage(0.10, 100));
    rec.record(&usage(0.25, 300));
    let mut after_restart = UsageRecorder::new(file.clone());
    after_restart.record(&usage(0.25, 300));
    after_restart.record(&usage(0.40, 500));
    let samples: Vec<UsageSample> = agentvm::jsonl::read(&file);
    assert_eq!(samples.iter().map(|s| s.output_tokens).collect::<Vec<_>>(), [100, 300, 500]);
    assert!(samples.windows(2).all(|w| w[0].at <= w[1].at));
}
