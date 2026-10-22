#!/usr/bin/env python3
"""Minimal FTP server for the Zen corpus (tests/cases/09_socket_hydra.z).

Listens on 127.0.0.1:2121 and accepts exactly one credential pair:
admin / s3cret99.  Any other password is rejected with a 530 reply, which is
what the hydra module uses to classify an attempt as a failure.

Usage:  python3 tests/mock_ftp.py [port]
"""

import socket
import sys
import threading

HOST = "127.0.0.1"
USER = "admin"
PASSWORD = "s3cret99"
BANNER = b"220 Zen mock FTP server ready.\r\n"


def reply(conn, line: bytes) -> None:
    conn.sendall(line + b"\r\n")


def handle(conn: socket.socket) -> None:
    conn.sendall(BANNER)
    user = None
    buf = b""
    try:
        conn.settimeout(10)
        while True:
            while b"\r\n" not in buf:
                chunk = conn.recv(4096)
                if not chunk:
                    return
                buf += chunk
            line, buf = buf.split(b"\r\n", 1)
            text = line.decode("utf-8", "replace")
            cmd = text.split(" ", 1)[0].upper()
            if cmd == "USER":
                user = text[5:] if len(text) > 5 else ""
                reply(conn, b"331 Password required")
            elif cmd == "PASS":
                pw = text[5:] if len(text) > 5 else ""
                if user == USER and pw == PASSWORD:
                    reply(conn, b"230 User logged in")
                else:
                    reply(conn, b"530 Login incorrect")
            elif cmd == "QUIT":
                reply(conn, b"221 Goodbye")
                return
            else:
                reply(conn, b"500 Command not understood")
    except (OSError, socket.timeout):
        pass
    finally:
        try:
            conn.close()
        except OSError:
            pass


def main() -> None:
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 2121
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind((HOST, port))
    server.listen(16)
    sys.stderr.write(f"mock_ftp listening on {HOST}:{port}\n")
    sys.stderr.flush()
    try:
        while True:
            conn, _ = server.accept()
            threading.Thread(target=handle, args=(conn,), daemon=True).start()
    except KeyboardInterrupt:
        pass
    finally:
        server.close()


if __name__ == "__main__":
    main()