"""Minimal mock TLS server for testing. Serves tests/fixtures/lab.pem on
127.0.0.1:2443: completes the handshake, then closes. Started by
tests/run_tests.sh like mock_ftp.py; hermetic (localhost only)."""
import os
import socket
import ssl
import threading

HOST, PORT = "127.0.0.1", 2443
HERE = os.path.dirname(os.path.abspath(__file__))


def handle(conn):
    try:
        conn.recv(1024)
    except Exception:
        pass
    finally:
        try:
            conn.shutdown(socket.SHUT_RDWR)
        except Exception:
            pass
        conn.close()


def main():
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(
        os.path.join(HERE, "fixtures", "lab.pem"),
        os.path.join(HERE, "fixtures", "lab-key.pem"),
    )
    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind((HOST, PORT))
    srv.listen(16)
    while True:
        conn, _ = srv.accept()
        try:
            tls = ctx.wrap_socket(conn, server_side=True)
        except Exception:
            conn.close()
            continue
        threading.Thread(target=handle, args=(tls,), daemon=True).start()


if __name__ == "__main__":
    main()
