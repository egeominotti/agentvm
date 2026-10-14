"""Forward actual S3 bytes unchanged, including signed headers and responses."""

import json
import select
import socket
import sys
import threading
from pathlib import Path

listen_port, target_port = map(int, sys.argv[1:3])
if listen_port in (7777, 9100) or target_port == 7777:
    raise ValueError("The proxy must use an isolated listening port")
listener = socket.socket()
listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
listener.bind(("127.0.0.1", listen_port))
listener.listen()
Path(sys.argv[3]).write_text(json.dumps({"port": listener.getsockname()[1]}))


def relay(client):
    upstream = None
    try:
        upstream = socket.create_connection(("127.0.0.1", target_port), timeout=10)
        while True:
            readable, _, _ = select.select([client, upstream], [], [], 10)
            for source in readable:
                data = source.recv(65536)
                if not data:
                    return
                (upstream if source is client else client).sendall(data)
    except OSError:
        pass
    finally:
        client.close()
        if upstream is not None:
            upstream.close()


while True:
    client, _ = listener.accept()
    threading.Thread(target=relay, args=(client,), daemon=True).start()
