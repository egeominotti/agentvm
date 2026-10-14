"""One length-framed JSON operation per connection over the existing vsock bridge."""
import json
import struct
import agent_exec
import agent_files
import agent_terminal

FRAME_LIMIT = 16 * 1024 * 1024


def dispatch(op, args):
    if not isinstance(op, str) or not isinstance(args, dict):
        raise ValueError("request requires op string and args object")
    if op == "capabilities":
        return {"version": 1, "exec": True, "files": True, "terminal": True, "frame_limit": FRAME_LIMIT}
    group, _, action = op.partition(".")
    handler = {"exec": agent_exec, "files": agent_files, "terminal": agent_terminal}.get(group)
    if handler is None:
        raise ValueError("unsupported control operation")
    return handler.dispatch(action, args)


def read_exact(conn, length):
    data = bytearray()
    while len(data) < length:
        part = conn.recv(min(65536, length - len(data)))
        if not part:
            raise ValueError("incomplete control frame")
        data.extend(part)
    return data


def serve(conn):
    try:
        conn.settimeout(30)
        length = struct.unpack(">I", read_exact(conn, 4))[0]
        if length > FRAME_LIMIT:
            raise ValueError("control frame exceeds 16 MiB")
        request = json.loads(read_exact(conn, length))
        if not isinstance(request, dict):
            raise ValueError("control request must be an object")
        reply = {"ok": True, "result": dispatch(request.get("op"), request.get("args", {}))}
    except (ValueError, OSError, TypeError, KeyError) as error:
        reply = {"ok": False, "error": str(error)[:4096]}
    try:
        payload = json.dumps(reply, separators=(",", ":")).encode()
        if len(payload) > FRAME_LIMIT:
            payload = b'{"ok":false,"error":"control response exceeds 16 MiB"}'
        conn.sendall(struct.pack(">I", len(payload)) + payload)
    except OSError:
        pass
    finally:
        conn.close()
