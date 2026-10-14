"""Real guest processes, binary files and framed connections."""
import base64
import importlib
import json
import os
import socket
import struct
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))


class ControlTests(unittest.TestCase):
    def setUp(self):
        self.control = importlib.import_module("agent_control")
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def call(self, op, **args):
        return self.control.dispatch(op, args)

    def wait(self, execution):
        for _ in range(300):
            result = self.call("exec.read", exec_id=execution["exec_id"])
            if result["state"] != "running":
                return result
            time.sleep(0.01)
        self.fail("execution did not finish")

    def test_literal_argv_and_stdin(self):
        run = self.call("exec.start", argv=[sys.executable, "-c", "import sys; print(sys.argv[1]); print(sys.stdin.read())", "$(touch NEVER)"], cwd=self.tmp.name, stdin="hello")
        out = self.wait(run)
        self.assertEqual(base64.b64decode(out["stdout"]), b"$(touch NEVER)\nhello\n")
        self.assertEqual(out["exit_code"], 0)
        self.assertFalse(Path(self.tmp.name, "NEVER").exists())

    def test_timeout_and_cancel_finish_processes(self):
        for cancel in (False, True):
            run = self.call("exec.start", argv=[sys.executable, "-c", "import time; time.sleep(30)"], cwd=self.tmp.name, timeout_s=1)
            if cancel:
                self.call("exec.cancel", exec_id=run["exec_id"])
            out = self.wait(run)
            self.assertEqual(out["state"], "cancelled" if cancel else "timed_out")
            self.assertIsNotNone(out["exit_code"])

    def test_running_output_is_available_before_process_finishes(self):
        run = self.call("exec.start", argv=[sys.executable, "-c", "import time; print('ready', flush=True); time.sleep(30)"], cwd=self.tmp.name)
        try:
            for _ in range(50):
                out = self.call("exec.read", exec_id=run["exec_id"])
                if base64.b64decode(out["stdout"]) == b"ready\n":
                    break
                time.sleep(0.01)
            self.assertEqual(base64.b64decode(out["stdout"]), b"ready\n")
            self.assertEqual(out["state"], "running")
        finally:
            self.call("exec.cancel", exec_id=run["exec_id"])
            self.wait(run)

    def test_output_is_bounded(self):
        out = self.wait(self.call("exec.start", argv=[sys.executable, "-c", "import sys; sys.stdout.write('x'*10000); sys.stderr.write('y'*10000)"], cwd=self.tmp.name, output_limit=64))
        self.assertEqual(len(base64.b64decode(out["stdout"])), 64)
        self.assertEqual(len(base64.b64decode(out["stderr"])), 64)
        self.assertTrue(out["stdout_truncated"])

    def test_cancel_kills_the_entire_process_group(self):
        script = "import subprocess,time; child=subprocess.Popen(['sleep','30']); print(child.pid, flush=True); time.sleep(30)"
        run = self.call("exec.start", argv=[sys.executable, "-c", script], cwd=self.tmp.name)
        try:
            for _ in range(100):
                out = self.call("exec.read", exec_id=run["exec_id"])
                if out["stdout"]:
                    break
                time.sleep(0.01)
            pid = int(base64.b64decode(out["stdout"]).strip())
            self.call("exec.cancel", exec_id=run["exec_id"])
            self.wait(run)
            import subprocess
            status = subprocess.run(["ps", "-o", "stat=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
            self.assertTrue(not status or status.startswith("Z"), status)
        finally:
            self.call("exec.cancel", exec_id=run["exec_id"])
            self.wait(run)

    def test_binary_files_offset_and_directory_operations(self):
        path = str(Path(self.tmp.name, "dir"))
        self.call("files.mkdir", path=path)
        file = path + "/binary"
        self.call("files.write", path=file, data=base64.b64encode(b"\x00\xffAB").decode(), offset=0, truncate=True)
        self.call("files.write", path=file, data=base64.b64encode(b"C").decode(), offset=3)
        out = self.call("files.read", path=file, offset=1, size=3)
        self.assertEqual(base64.b64decode(out["data"]), b"\xffAC")
        self.assertEqual(self.call("files.stat", path=file)["size"], 4)
        self.assertEqual(self.call("files.list", path=path)["entries"][0]["name"], "binary")
        self.call("files.rename", path=file, destination=file + "2")
        self.call("files.remove", path=file + "2")
        self.assertEqual(self.call("files.list", path=path)["entries"], [])

    def test_validation_rejects_unsafe_bounds_and_sessions(self):
        for op, args in [("exec.start", {"argv": []}), ("exec.start", {"argv": ["true"], "timeout_s": 0}), ("files.read", {"path": self.tmp.name, "size": 2**25}), ("terminal.input", {"session": "; touch BAD", "text": "hello"})]:
            with self.assertRaises(ValueError):
                self.control.dispatch(op, args)

    def test_framed_oversized_requests_are_rejected_without_reading_body(self):
        left, right = socket.socketpair()
        thread = threading.Thread(target=self.control.serve, args=(right,))
        thread.start()
        left.sendall(struct.pack(">I", 16 * 1024 * 1024 + 1))
        length = struct.unpack(">I", left.recv(4))[0]
        result = json.loads(left.recv(length))
        self.assertFalse(result["ok"])
        self.assertIn("exceeds", result["error"])
        left.close()
        thread.join(2)
        self.assertFalse(thread.is_alive())

    def test_framed_capabilities_and_errors(self):
        for request in ({"op": "capabilities", "args": {}}, {"op": "no_such_operation", "args": {}}):
            left, right = socket.socketpair()
            thread = threading.Thread(target=self.control.serve, args=(right,))
            thread.start()
            payload = json.dumps(request).encode()
            left.sendall(struct.pack(">I", len(payload)) + payload)
            length = struct.unpack(">I", left.recv(4))[0]
            result = json.loads(left.recv(length))
            self.assertEqual(result["ok"], request["op"] == "capabilities")
            left.close()
            thread.join(2)
            self.assertFalse(thread.is_alive())


if __name__ == "__main__":
    unittest.main()
