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

/// The conversation copied out of the VM, a page at a time, still there once the VM is gone.
#[test]
fn the_conversation_is_read_from_the_copied_sessions() {
    use agentvm::app::history::conversation;
    use agentvm::domain::transcript::EntryKind;
    let (home, repo) = (tempfile::tempdir().unwrap(), crate::helpers::git_repo());
    let id = crate::helpers::running_task(home.path(), &repo, true);
    let ctx = crate::helpers::ctx(home.path());
    ctx.store.insert(agentvm::app::store::Store::load(&home.path().join("jobs")).remove(0));
    let claude = home.path().join("jobs").join(id.as_str()).join("share/claude");
    std::fs::create_dir_all(&claude).unwrap();
    let lines = [
        serde_json::json!({"type": "user", "timestamp": "t0", "message": {"role": "user", "content": "Add a README"}}),
        serde_json::json!({"type": "ai-title", "aiTitle": "README"}),
        serde_json::json!({"type": "assistant", "timestamp": "t1", "message": {"id": "m1", "content": [{"type": "tool_use", "name": "Write", "input": {"file_path": "README.md"}}]}}),
    ];
    std::fs::write(claude.join("p--s.jsonl"), lines.iter().map(|l| l.to_string() + "\n").collect::<String>()).unwrap();
    let page = conversation(&ctx, &id, 0).unwrap();
    assert_eq!(page.entries.len(), 2);
    assert_eq!(page.entries[0].kind, EntryKind::User { text: "Add a README".into() });
    assert_eq!(page.entries[1].kind, EntryKind::ToolUse { name: "Write".into(), input: "README.md".into() });
    assert_eq!(page.next, 3);
    assert!(conversation(&ctx, &id, 3).unwrap().entries.is_empty());
}
