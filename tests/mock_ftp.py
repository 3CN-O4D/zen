"""Minimal mock FTP server for testing. Listens on 127.0.0.1:2121, creds admin/s3cret99."""
import socket
import threading

HOST, PORT = "127.0.0.1", 2121
USER, PASS = "admin", "s3cret99"

def handle(conn, addr):
    try:
        conn.sendall(b"220 MockFTP ready\r\n")
        user_ok = False
        buf = b""
        while True:
            data = conn.recv(1024)
            if not data:
                break
            buf += data
            while b"\r\n" in buf:
                line, buf = buf.split(b"\r\n", 1)
                cmd = line.decode(errors="replace").strip()
                verb = cmd.split(" ")[0].upper() if cmd else ""
                if verb == "USER":
                    user_ok = cmd.split(" ", 1)[1] == USER if len(cmd) > 5 else False
                    conn.sendall(b"331 Password required\r\n")
                elif verb == "PASS":
                    ok = user_ok and len(cmd) > 5 and cmd.split(" ", 1)[1] == PASS
                    conn.sendall(b"230 Login successful\r\n" if ok else b"530 Login incorrect\r\n")
                elif verb == "QUIT":
                    conn.sendall(b"221 Bye\r\n")
                    conn.close()
                    return
                elif verb in ("PWD", "SYST", "TYPE", "CWD", "PASV"):
                    conn.sendall(b"257 \"/\" is current directory\r\n" if verb == "PWD" else b"200 OK\r\n")
                elif verb == "LIST" or verb == "NLST":
                    conn.sendall(b"150 Here comes the listing\r\n226 Transfer done\r\n")
                else:
                    conn.sendall(b"502 Not implemented\r\n")
    except Exception:
        pass
    finally:
        try:
            conn.close()
        except Exception:
            pass

def main():
    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind((HOST, PORT))
    srv.listen(8)
    print(f"MockFTP listening on {HOST}:{PORT}", flush=True)
    while True:
        conn, addr = srv.accept()
        threading.Thread(target=handle, args=(conn, addr), daemon=True).start()

if __name__ == "__main__":
    main()
