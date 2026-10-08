//! External commands with a time limit: a git waiting on a lock or a Keychain waiting for an
//! unlock dialog is killed at the limit, never waited on forever by the server.

use std::io::{self, Read};
use std::process::{Child, Command, Output, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Runs `cmd` (stdin closed) and collects its output, killing it if it outlives `limit`.
pub fn output(cmd: &mut Command, limit: Duration) -> io::Result<Output> {
    let child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    wait_output(child, limit, &program(cmd))
}

/// Waits for a child spawned with piped stdout and stderr, killing it if it outlives `limit`.
/// Both pipes are drained meanwhile: a chatty command never blocks on a full pipe.
pub fn wait_output(mut child: Child, limit: Duration, name: &str) -> io::Result<Output> {
    let drain = |pipe: Option<Box<dyn Read + Send>>| -> JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            buf
        })
    };
    let stdout = drain(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let stderr = drain(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let t0 = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if t0.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            let secs = limit.as_secs_f32();
            tracing::warn!(command = name, secs, "an external command did not finish in time: killed");
            return Err(io::Error::new(io::ErrorKind::TimedOut, format!("{name} did not finish in {secs:.0} s")));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Ok(Output { status, stdout: stdout.join().unwrap_or_default(), stderr: stderr.join().unwrap_or_default() })
}

/// `cmd.output_within(limit)`: `Command::output` with a time limit.
pub trait OutputWithin {
    fn output_within(&mut self, limit: Duration) -> io::Result<Output>;
}

impl OutputWithin for Command {
    fn output_within(&mut self, limit: Duration) -> io::Result<Output> {
        output(self, limit)
    }
}

fn program(cmd: &Command) -> String {
    let args: Vec<_> = cmd.get_args().take(2).map(|a| a.to_string_lossy().into_owned()).collect();
    format!("{} {}", cmd.get_program().to_string_lossy(), args.join(" ")).trim().to_owned()
}
