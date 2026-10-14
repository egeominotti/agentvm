"""Chunked binary operations only inside the guest's filesystem."""
import base64
import os
import shutil
import stat
from pathlib import Path
from agent_exec import integer

CHUNK = 1024 * 1024


def regular_file(path, flags, mode):
    fd = os.open(path, flags | os.O_NONBLOCK, 0o600)
    try:
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            raise ValueError("file transfers require a regular guest file")
        return os.fdopen(fd, mode)
    except Exception:
        os.close(fd)
        raise


def dispatch(op, args):
    name = args.get("path")
    if not isinstance(name, str) or not name or "\x00" in name:
        raise ValueError("path must be a nonempty guest path")
    path = Path(name)
    if not path.is_absolute():
        path = Path("/root/work") / path
    if op == "read":
        offset = integer(args, "offset", 0, 0, 2**63 - 1)
        size = integer(args, "size", CHUNK, 1, CHUNK)
        with regular_file(path, os.O_RDONLY, "rb") as file:
            file.seek(offset)
            data = file.read(size)
            eof = not file.read(1)
        return {"data": base64.b64encode(data).decode(), "offset": offset, "size": len(data), "eof": eof}
    if op == "write":
        offset = integer(args, "offset", 0, 0, 2**63 - 1)
        encoded = args.get("data")
        if not isinstance(encoded, str) or len(encoded) > ((CHUNK + 2) // 3) * 4:
            raise ValueError("data must be a base64 chunk at most 1 MiB")
        try:
            data = base64.b64decode(encoded, validate=True)
        except ValueError as error:
            raise ValueError("invalid base64 data") from error
        if len(data) > CHUNK:
            raise ValueError("data exceeds 1 MiB")
        flags = os.O_WRONLY | os.O_CREAT | (os.O_TRUNC if args.get("truncate", False) else 0)
        with regular_file(path, flags, "wb") as file:
            file.seek(offset)
            file.write(data)
        return {"written": len(data), "offset": offset}
    if op == "stat":
        info = path.stat()
        return {"size": info.st_size, "directory": path.is_dir(), "mode": info.st_mode & 0o777, "mtime": info.st_mtime}
    if op == "list":
        limit = integer(args, "limit", 1000, 1, 1000)
        offset = integer(args, "offset", 0, 0, 2**31 - 1)
        # Scanning is bounded even when a directory holds millions of entries.
        entries = []
        with os.scandir(path) as scan:
            for index, entry in enumerate(scan):
                if index < offset:
                    continue
                if len(entries) == limit:
                    return {"entries": entries, "next_offset": index}
                entries.append({"name": entry.name, "directory": entry.is_dir(follow_symlinks=False)})
        return {"entries": entries, "next_offset": None}
    if op == "mkdir":
        path.mkdir(parents=bool(args.get("parents", False)), exist_ok=bool(args.get("exist_ok", False)))
    elif op == "remove":
        if path.is_dir() and not path.is_symlink():
            shutil.rmtree(path) if args.get("recursive", False) else path.rmdir()
        else:
            path.unlink()
    elif op == "rename":
        destination = args.get("destination")
        if not isinstance(destination, str) or not destination or "\x00" in destination:
            raise ValueError("destination must be a guest path")
        target = Path(destination)
        path.rename(target if target.is_absolute() else Path("/root/work") / target)
    else:
        raise ValueError("unsupported file operation")
    return {"ok": True}
