import socket
import sys
import time

SOCK = sys.argv[1] if len(sys.argv) > 1 else "out/modem.sock"

RESPONSES = {
    "AT": [],
    "ATE0": [],
    "ATE1": [],
    "AT+CGMI": ["Quectel"],
    "AT+CGMM": ["EG25-G"],
    "AT+CPIN?": ["+CPIN: READY"],
    "AT+CSQ": ["+CSQ: 21,99"],
    "AT+CREG?": ["+CREG: 0,1"],
}


def connect(path):
    for _ in range(120):
        conn = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            conn.connect(path)
            return conn
        except OSError:
            conn.close()
            time.sleep(1)
    return None


def main():
    conn = connect(SOCK)
    if conn is None:
        print("modem-emu: connection failed", flush=True)
        sys.exit(1)
    print("modem-emu: modem connected", flush=True)
    buf = b""
    echo = True
    while True:
        try:
            data = conn.recv(256)
        except OSError:
            break
        if not data:
            break
        buf += data
        while b"\r" in buf:
            raw, buf = buf.split(b"\r", 1)
            cmd = raw.decode(errors="ignore").strip()
            if not cmd:
                continue
            if echo:
                conn.sendall(raw + b"\r\n")
            if cmd.startswith("ATE"):
                echo = cmd == "ATE1"
            if cmd in RESPONSES:
                payload = "".join(f"\r\n{line}\r\n" for line in RESPONSES[cmd]) + "\r\nOK\r\n"
            else:
                payload = "\r\nERROR\r\n"
            conn.sendall(payload.encode())


main()
