#!/usr/bin/env python3
"""PTY driver for yarp end-to-end tests.

Runs yarp on a real pty and behaves like a terminal: answers cursor-position
(DSR) and device-attribute queries that line editors such as PSReadLine
require, and types each line only after the shell integration has emitted a
fresh prompt marker (OSC 133;A) — quiet-time pacing alone races slow-starting
shells like pwsh, whose cold start has output gaps before the first prompt.

usage: driver.py <yarp-binary> <shell> <line> [<line> ...]
"""
import fcntl
import os
import pty
import select
import struct
import sys
import termios
import time

PROMPT_MARK = b"\x1b]133;A"
SETTLE_SECS = 0.3      # let the line editor finish drawing after the mark
HARD_TIMEOUT = 120.0


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
    dsr_answered = 0
    da_answered = 0

    while time.time() < deadline:
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
            # Answer terminal queries like a real emulator (counted against
            # the whole buffer so split sequences aren't missed).
            dsr = out.count(b"\x1b[6n")
            while dsr_answered < dsr:
                os.write(fd, b"\x1b[24;1R")
                dsr_answered += 1
            da = out.count(b"\x1b[c") + out.count(b"\x1b[0c")
            while da_answered < da:
                os.write(fd, b"\x1b[?6c")
                da_answered += 1
            continue
        # Type line N only after prompt N+1 has been drawn and settled.
        if next_line < len(lines):
            prompts = out.count(PROMPT_MARK)
            if prompts > next_line and time.time() - last_output > SETTLE_SECS:
                os.write(fd, lines[next_line].encode() + b"\r")
                next_line += 1
        elif time.time() - last_output > 5.0:
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
