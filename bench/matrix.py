#!/usr/bin/env python3
"""Run matched direct and proxied trials with preset workload budgets."""

import argparse
import json
from pathlib import Path
import random
import re
import sys

from run import run


# (name, rate, concurrency, p99 budget, additional flags)
CASES = [
    ("churn", 1000, 512, "10ms", ""),
    ("churn", 4000, 512, "10ms", ""),
    ("churn", 8000, 1024, "10ms", ""),
    ("churn", 12000, 2048, "10ms", ""),
    ("persistent", 1000, 64, "10ms", "-persistent"),
    ("persistent", 4000, 64, "10ms", "-persistent"),
    ("persistent", 8000, 512, "10ms", "-persistent"),
    ("delay10", 1000, 256, "25ms", "-delay 10ms"),
    ("delay10", 4000, 512, "25ms", "-delay 10ms"),
    ("delay100", 1000, 512, "125ms", "-delay 100ms"),
    ("delay100", 4000, 1024, "125ms", "-delay 100ms"),
    ("delay1000", 250, 1024, "1100ms", "-delay 1s"),
    ("delay1000", 1000, 4096, "1100ms", "-delay 1s"),
    ("mixed", 1000, 512, "1100ms", "-mixed"),
    ("mixed", 4000, 1024, "1100ms", "-mixed"),
    ("slow-one", 1000, 512, "250ms", "-slow 1"),
    ("slow-several", 4000, 2048, "250ms", "-slow 4"),
    ("refused", 1000, 512, "25ms", "-refused 1"),
    ("blackhole", 1000, 2048, "25ms", ""),
    ("reset", 1000, 512, "25ms", "-reset 1"),
    ("restore", 1000, 512, "25ms", "-refused 1 -restore 5s -soak-rate 1000 -soak 5s -recovery 3s"),
    ("upload", 100, 64, "100ms", "-bytes 4194304 -direction upload"),
    ("download", 100, 64, "100ms", "-bytes 4194304 -direction download"),
    ("bidir", 100, 64, "100ms", "-bytes 4194304"),
    ("bidir", 400, 256, "100ms", "-bytes 4194304"),
    ("slow-reader", 50, 512, "750ms", "-bytes 2097152 -direction download -read-pause 5ms"),
    ("slow-writer", 50, 512, "750ms", "-bytes 2097152 -direction upload -write-pause 5ms"),
    ("idle", 250, 2048, "2200ms", "-idle 2s"),
    ("long-lived", 1000, 64, "25ms", "-persistent -soak-rate 1000 -soak 32s -recovery 5s"),
    ("overload", 300, 8192, "1200ms", "-delay 1s -soak-rate 6500 -soak 35s -recovery 8s"),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tool", required=True)
    parser.add_argument("--proxy", action="append", default=[], help="proxy binary, repeat for multiple builds")
    parser.add_argument("--direct", action="store_true", help="also run matched direct-to-backend trials")
    parser.add_argument("--out", type=Path, required=True, help="append trial results as JSON Lines")
    parser.add_argument("--match", default=".", help="only run workload names matching this regular expression")
    parser.add_argument("--repeats", type=int, default=2)
    parser.add_argument("--warmup", default="2s")
    parser.add_argument("--duration", default="6s")
    parser.add_argument("--log", default="error")
    parser.add_argument("--sample-interval", type=float, default=2)
    parser.add_argument("--cooldown", type=float, default=2)
    parser.add_argument("--sockets", action="store_true")
    args = parser.parse_args()
    routes = [None] * args.direct + args.proxy
    if not routes or args.repeats < 1 or args.sample_interval <= 0 or args.cooldown < 0:
        parser.error("select at least one route and use valid repetitions and sample times")

    cases = [case for case in CASES if re.search(args.match, case[0])]
    if not cases:
        parser.error("no workloads matched")
    with args.out.open("a", buffering=1) as report:
        for trial in range(1, args.repeats + 1):
            order = [(case, route) for case in cases for route in routes]
            random.Random(trial).shuffle(order)
            for (name, rate, concurrency, budget, extra), route in order:
                print(f"{trial} {name} {rate}/s {route or 'direct'}", file=sys.stderr, flush=True)
                args.proxy = route
                args.blackhole = int(name == "blackhole")
                flags = ["-rate", str(rate), "-concurrency", str(concurrency), "-p99-budget", budget,
                         "-warmup", args.warmup, "-duration", args.duration, *extra.split()]
                try:
                    record = run(args, flags)
                except SystemExit as error:
                    if not args.blackhole or "did not blackhole connects" not in str(error):
                        raise
                    record = {"proxy": route, "flags": flags, "unavailable": str(error)}
                print(json.dumps({"workload": name, "trial": trial, **record}), file=report)


if __name__ == "__main__":
    main()
