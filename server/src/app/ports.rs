//! Keeps one forward per listening VM port, following what the VM reports every second.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::adapters::forward::PortForward;
use crate::adapters::jobdir::JobWorkspace;
use crate::domain::ids::TaskId;
use crate::domain::metrics::{ForwardedPort, ListeningPort};

/// Forwards of one VM: guest port → (forward, owning process name).
type TaskForwards = HashMap<u16, (PortForward, String)>;

#[derive(Default)]
pub struct PortForwards {
    by_task: Mutex<HashMap<TaskId, TaskForwards>>,
}

impl PortForwards {
    /// Starts forwards for new ports, stops those that disappeared; returns the current list.
    pub fn sync(&self, jobs: &std::path::Path, id: &TaskId, listening: &[ListeningPort]) -> Vec<ForwardedPort> {
        let mut all = self.by_task.lock().unwrap();
        let current = all.entry(id.clone()).or_default();
        current.retain(|port, _| listening.iter().any(|l| l.port == *port));
        for l in listening {
            if !current.contains_key(&l.port)
                && let Ok(fwd) = PortForward::start(l.port, JobWorkspace::pty_socket_of(jobs, id))
            {
                current.insert(l.port, (fwd, l.name.clone()));
            }
        }
        let mut list: Vec<ForwardedPort> = current
            .iter()
            .map(|(port, (fwd, name))| ForwardedPort { port: *port, host_port: fwd.host_port, name: name.clone() })
            .collect();
        list.sort_by_key(|p| p.port);
        list
    }

    pub fn stop_all(&self, id: &TaskId) {
        self.by_task.lock().unwrap().remove(id);
    }
}
