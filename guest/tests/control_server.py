"""Run the production guest connection handler on a Unix socket for host transport tests."""
import runpy
import socket
import sys
import threading
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
serve = runpy.run_path(str(Path(__file__).resolve().parents[1] / "agentvm-pty"))["serve"]
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(sys.argv[1])
server.listen(128)
print("ready", flush=True)
while True:
    connection, _ = server.accept()
    threading.Thread(target=serve, args=(connection,), daemon=True).start()
