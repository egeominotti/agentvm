"""Bounded asynchronous literal-argv executions, independent of terminal sessions."""
import base64
import os
import signal
import subprocess
import threading
import uuid

LIMIT = 1024 * 1024
RUNS = {}
LOCK = threading.Lock()


def integer(args, name, default, low, high):
    value = args.get(name, default)
    if type(value) is not int or not low <= value <= high:
        raise ValueError(f"{name} must be between {low} and {high}")
    return value


def kill_group(process):
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


class Execution:
    def __init__(self, args):
        argv = args.get("argv")
        if not isinstance(argv, list) or not argv or not all(isinstance(a, str) and "\x00" not in a for a in argv):
            raise ValueError("argv must be a nonempty array of strings without NUL bytes")
        self.timeout = integer(args, "timeout_s", 60, 1, 1800)
        self.limit = integer(args, "output_limit", LIMIT, 1, LIMIT)
        stdin = args.get("stdin", "")
        if not isinstance(stdin, str) or len(stdin.encode()) > LIMIT:
            raise ValueError("stdin must be text at most 1 MiB")
        self.lock = threading.Lock()
        self.state = "running"
        self.exit_code = None
        self.output = {"stdout": bytearray(), "stderr": bytearray()}
        self.truncated = {"stdout": False, "stderr": False}
        self.process = subprocess.Popen(argv, cwd=args.get("cwd", "/root/work"), stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
        threading.Thread(target=self.collect, args=(stdin.encode(),), daemon=True).start()

    def drain(self, stream, name):
        with stream:
            while True:
                chunk = os.read(stream.fileno(), 65536)
                if not chunk:
                    return
                with self.lock:
                    room = self.limit - len(self.output[name])
                    self.output[name].extend(chunk[:room])
                    self.truncated[name] |= len(chunk) > room

    def feed(self, data):
        try:
            with self.process.stdin:
                self.process.stdin.write(data)
        except (BrokenPipeError, OSError):
            pass

    def collect(self, stdin):
        readers = [threading.Thread(target=self.drain, args=(getattr(self.process, n), n), daemon=True)
                   for n in ("stdout", "stderr")]
        for thread in readers:
            thread.start()
        writer = threading.Thread(target=self.feed, args=(stdin,), daemon=True)
        writer.start()
        try:
            self.process.wait(timeout=self.timeout)
        except subprocess.TimeoutExpired:
            with self.lock:
                if self.state == "running":
                    self.state = "timed_out"
                kill_group(self.process)
            self.process.wait()
        # Descendants must not retain pipes or outlive a completed execution.
        kill_group(self.process)
        writer.join()
        for thread in readers:
            thread.join()
        with self.lock:
            self.exit_code = self.process.returncode
            if self.state == "running":
                self.state = "completed"

    def cancel(self):
        with self.lock:
            if self.exit_code is None:
                self.state = "cancelled"
                kill_group(self.process)
        return self.read()

    def read(self):
        with self.lock:
            return {"state": self.state if self.exit_code is not None else "running", "exit_code": self.exit_code,
                    **{name: base64.b64encode(data).decode() for name, data in self.output.items()},
                    **{name + "_truncated": value for name, value in self.truncated.items()}}


def dispatch(op, args):
    if op == "start":
        with LOCK:
            # Retain a bounded set of completed executions; never evict live ones.
            while len(RUNS) >= 128:
                finished = next((key for key, run in RUNS.items() if run.exit_code is not None), None)
                if finished is None:
                    raise ValueError("too many active executions")
                del RUNS[finished]
            key = str(uuid.uuid4())
            RUNS[key] = Execution(args)
        return {"exec_id": key, "state": "running"}
    with LOCK:
        run = RUNS.get(args.get("exec_id"))
    if run is None:
        raise ValueError("execution not found")
    if op == "read":
        return run.read()
    if op == "cancel":
        return run.cancel()
    raise ValueError("unsupported execution operation")
