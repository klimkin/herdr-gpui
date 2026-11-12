#!/usr/bin/python3

import pathlib
import sys
import time


rate = int(sys.argv[1])
count = pathlib.Path(f"{__file__}.count")
batch = rate // 20
deadline = time.monotonic() + 8
next_tick = time.monotonic()
emitted = 0
while time.monotonic() < deadline:
    for _ in range(batch):
        sys.stdout.write(f"L{emitted:06d} " + "x" * 64 + "\n")
        emitted += 1
    sys.stdout.flush()
    count.write_text(str(emitted))
    next_tick += 0.05
    time.sleep(max(0, next_tick - time.monotonic()))
print("LOAD_DONE")
count.write_text(str(emitted))
