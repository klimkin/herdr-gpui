#!/usr/bin/python3

import pathlib
import sys
import time


index = int(sys.argv[1])
rate = int(sys.argv[2])
batch = max(1, rate // 20)
counter = pathlib.Path(f"{__file__}.agent{index}.count")
deadline = time.monotonic() + 18
next_tick = time.monotonic()
emitted = 0
while time.monotonic() < deadline:
    for _ in range(batch):
        sys.stdout.write(f"A{index}:{emitted:06d} " + "x" * 20 + "\n")
        emitted += 1
    sys.stdout.flush()
    counter.write_text(str(emitted))
    next_tick += 0.05
    time.sleep(max(0, next_tick - time.monotonic()))
print(f"A{index}_DONE")
counter.write_text(str(emitted))
