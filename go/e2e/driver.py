#!/usr/bin/env python3
"""PTY driver for yarp end-to-end tests.

Runs yarp on a real pty and behaves like a terminal: answers cursor-position
(DSR) and device-attribute queries that line editors such as PSReadLine
require, types the given lines when output goes quiet, and exits with the
child. Piping stdin into a shell is not faithful — modern shells query the
terminal and hang or garble without answers.

usage: driver.py <yarp-binary> <shell> <line> [<line> ...]
"""
import fcntl
import os
import pty
import struct
import sys
import termios
import time

QUIET_SECS = 0.8       # send the next line once output pauses this long
HARD_TIMEOUT = 90.0


def main():
    yarp, shell, *lines = sys.argv[1:]
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["SHELL"] = shell
        os.execv(yarp, [yarp])

    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))

    out = bytearray()
    deadline = time.time() + HARD_TIMEOUT
    last_output = time.time()
    next_line = 0
    sent_at = 0.0

    while time.time() < deadline:
        import select
        r, _, _ = select.select([fd], [], [], 0.1)
        if r:
            try:
                data = os.read(fd, 65536)
            except OSError:
                break
            if not data:
                break
            out.extend(data)
            last_output = time.time()
            # Answer terminal queries like a real emulator would.
            for _ in range(data.count(b"\x1b[6n")):
                os.write(fd, b"\x1b[24;1R")
            if b"\x1b[c" in data or b"\x1b[0c" in data:
                os.write(fd, b"\x1b[?6c")
            continue
        quiet = time.time() - last_output
        if next_line < len(lines) and quiet > QUIET_SECS and time.time() - sent_at > QUIET_SECS:
            os.write(fd, lines[next_line].encode() + b"\r")
            next_line += 1
            sent_at = time.time()
        if next_line >= len(lines) and quiet > 5.0:
            break  # child never exited; let the caller inspect output

    try:
        os.close(fd)
    except OSError:
        pass
    _, status = os.waitpid(pid, 0)
    sys.stdout.buffer.write(bytes(out))
    sys.exit(os.waitstatus_to_exitcode(status) if os.WIFEXITED(status) else 1)


if __name__ == "__main__":
    main()
