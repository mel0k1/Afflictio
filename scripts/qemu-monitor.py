import json
import socket
import sys


def main():
    path = sys.argv[1]
    command = sys.argv[2]
    args = sys.argv[3:]
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(path)
    reader = sock.makefile("rwb")
    json.loads(reader.readline())
    reader.write((json.dumps({"execute": "qmp_capabilities"}) + "\n").encode())
    reader.flush()
    json.loads(reader.readline())
    if command == "key":
        keys = [{"type": "qcode", "data": k} for k in args]
        request = {"execute": "send-key", "arguments": {"keys": keys}}
    elif command == "dump":
        request = {"execute": "screendump", "arguments": {"filename": args[0]}}
    else:
        sys.exit("unknown command")
    reader.write((json.dumps(request) + "\n").encode())
    reader.flush()
    while True:
        line = json.loads(reader.readline())
        if "error" in line:
            sys.exit(json.dumps(line["error"]))
        if "return" in line:
            break
    sock.close()


if __name__ == "__main__":
    main()
