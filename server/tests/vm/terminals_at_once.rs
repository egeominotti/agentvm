//! Many terminals opened at the same moment (a wall of previews, several tabs): each one answers.

use std::time::Duration;

use agentvm::adapters::pty::PtyConnection;
use agentvm::adapters::vm::{VmEvent, VmProcess};
use agentvm::domain::spec::TaskSpec;
use agentvm::secret::Secret;

use crate::helpers::{KillVms, config, helper, repo_with_bundle, workspace};
use crate::terminal::open_ready_shell;

#[tokio::test]
#[ignore = "requires the golden image"]
async fn forty_terminals_opened_at_once_all_answer() {
    let tmp = tempfile::tempdir().unwrap();
    let _vms = KillVms(tmp.path().to_path_buf());
    let ws = workspace(&tmp);
    let base = repo_with_bundle(&ws);
    ws.write_spec(&TaskSpec {
        id: "t".into(),
        prompt: String::new(),
        branch: "agent/t".into(),
        base_sha: base,
        timeout_s: 60,
        interactive: true,
        model: None,
        claude_version: None,
        restore: false,
    })
    .unwrap();
    ws.write_token(&Secret::new("sk-ant-oat01-not-a-real-token".into())).unwrap();
    let mut cfg = config(&ws);
    cfg.pty_socket = Some(ws.pty_socket());
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &cfg, &ws.dir().join("vm.events")).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));
    let _ready = open_ready_shell(&ws).await;

    for round in 0..3 {
        let opens = (0..40).map(|i| {
            let socket = ws.pty_socket();
            async move {
                // Half of them read-only, as the wall's previews are.
                let mut pty = PtyConnection::open_with(&socket, "shell", 80, 24, i % 2 == 0).await.ok()?;
                tokio::time::timeout(Duration::from_secs(8), pty.recv()).await.ok()?.ok()?.map(|_| ())
            }
        });
        let answered = futures::future::join_all(opens).await.iter().filter(|r| r.is_some()).count();
        assert_eq!(answered, 40, "round {round}: only {answered} of 40 terminals answered");
    }
}
