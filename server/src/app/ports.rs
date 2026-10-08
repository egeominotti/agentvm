//! What each VM serves, following the ports it reports every second: HTTP services are reached by
//! name through the proxy (`<port>.<vm>.localhost`), the others get a direct TCP forward.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::adapters::forward::{self, PortForward};
use crate::domain::hostname;
use crate::domain::ids::TaskId;
use crate::domain::metrics::{ForwardedPort, ListeningPort, PortKind};

/// At most this many ports per VM are exposed: a VM must not exhaust the Mac's descriptors.
const MAX_PORTS: usize = 32;

enum Reach {
    Probing,
    Http,
    Tcp(PortForward),
    Unreachable,
}

struct Exposed {
    name: String,
    reach: Reach,
}

type Ports = HashMap<u16, Exposed>;

#[derive(Default)]
pub struct PortForwards {
    by_task: Arc<Mutex<HashMap<TaskId, Ports>>>,
}

impl PortForwards {
    /// Follows the VM's listening ports (new ones are probed in the background) and returns what
    /// is reachable now. `vm` is the machine's DNS name, `server_port` the dashboard's port.
    pub fn sync(
        &self,
        socket: PathBuf,
        id: &TaskId,
        vm: &str,
        server_port: u16,
        proxy_token: &str,
        listening: &[ListeningPort],
    ) -> Vec<ForwardedPort> {
        let mut all = self.by_task.lock().unwrap();
        let current = all.entry(id.clone()).or_default();
        current.retain(|port, _| listening.iter().any(|l| l.port == *port));
        for l in listening {
            if current.len() >= MAX_PORTS || current.contains_key(&l.port) {
                continue;
            }
            current.insert(l.port, Exposed { name: l.name.clone(), reach: Reach::Probing });
            let (by_task, id, socket, port) = (self.by_task.clone(), id.clone(), socket.clone(), l.port);
            tokio::spawn(async move {
                let reach = if forward::speaks_http(&socket, port).await {
                    Reach::Http
                } else {
                    PortForward::start(port, socket).map_or(Reach::Unreachable, Reach::Tcp)
                };
                // The port may have closed, or the VM stopped, while it was being probed.
                if let Some(e) = by_task.lock().unwrap().get_mut(&id).and_then(|p| p.get_mut(&port))
                    && matches!(e.reach, Reach::Probing)
                {
                    e.reach = reach;
                }
            });
        }
        let mut list: Vec<ForwardedPort> = current
            .iter()
            .filter_map(|(port, e)| {
                let (kind, url, host_port) = match &e.reach {
                    Reach::Http => {
                        let url =
                            format!("{}/?agentvm_token={proxy_token}", hostname::proxy_url(*port, vm, server_port));
                        (PortKind::Http, Some(url), None)
                    }
                    Reach::Tcp(fwd) => (PortKind::Tcp, None, Some(fwd.host_port)),
                    Reach::Probing | Reach::Unreachable => return None,
                };
                Some(ForwardedPort { port: *port, name: e.name.clone(), kind, url, host_port })
            })
            .collect();
        list.sort_by_key(|p| p.port);
        list
    }

    pub fn stop_all(&self, id: &TaskId) {
        self.by_task.lock().unwrap().remove(id);
    }
}
