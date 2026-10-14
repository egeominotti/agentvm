"""Delay and disconnect real WebSocket traffic without producing application responses."""

import json
import select
import socket
import sys
import threading
import time
from pathlib import Path

target_port = int(sys.argv[1])
ready_path = Path(sys.argv[2])
cut_path = Path(sys.argv[3])
if target_port == 7777:
    raise ValueError("The user's engine cannot be used by this test proxy")

listener = socket.socket()
listener.bind(("127.0.0.1", 0))
listener.listen()
ready_path.write_text(json.dumps({"port": listener.getsockname()[1]}))


def relay(client):
    upstream = None
    try:
        client.settimeout(10)
        header = b""
        while b"\r\n\r\n" not in header:
            chunk = client.recv(4096)
            if not chunk:
                return
            header += chunk
            if len(header) > 65536:
                raise ValueError("Oversized upgrade headers")
        # The real engine validates its own listening port, including forwarded upgrades.
        fields, tail = header.split(b"\r\n\r\n", 1)
        lines = fields.split(b"\r\n")
        lines = [f"Host: 127.0.0.1:{target_port}".encode() if line.lower().startswith(b"host:") else line for line in lines]
        time.sleep(1)
        upstream = socket.create_connection(("127.0.0.1", target_port), timeout=10)
        upstream.sendall(b"\r\n".join(lines) + b"\r\n\r\n" + tail)
        while True:
            if cut_path.exists():
                cut_path.unlink(missing_ok=True)
                return
            readable, _, _ = select.select([client, upstream], [], [], 0.025)
            for source in readable:
                data = source.recv(65536)
                if not data:
                    return
                (upstream if source is client else client).sendall(data)
    except (OSError, ValueError):
        pass
    finally:
        client.close()
        if upstream is not None:
            upstream.close()


while True:
    client, _ = listener.accept()
    threading.Thread(target=relay, args=(client,), daemon=True).start()
