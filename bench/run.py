#!/usr/bin/env python3
"""Run a TCP benchmark trial and collect local process and backend samples."""

import argparse
from contextlib import ExitStack
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time


def wait_ready(process, ready, label):
    end = time.monotonic() + 5
    while time.monotonic() < end:
        if process.poll() is not None:
            raise RuntimeError(f"{label} exited with status {process.returncode}")
        if ready():
            return
        time.sleep(0.05)
    raise RuntimeError(f"{label} did not become ready")


def proxy_ready():
    try:
        with socket.create_connection(("127.0.0.1", 8080), timeout=0.1):
            return True
    except OSError:
        return False


def cpu_seconds(value):
    seconds = 0
    for part in value.split(":"):
        seconds = seconds * 60 + float(part)
    return seconds


def process_stats(processes, start, lsof):
    names = {str(process.pid): name for name, process in processes.items()}
    ps = subprocess.run(
        ["ps", "-o", "pid=,rss=,time=", "-p", ",".join(names)],
        capture_output=True, text=True, check=False,
    )
    sample = {"elapsed_s": round(time.monotonic() - start, 3)}
    for row in ps.stdout.splitlines():
        pid, rss, cpu = row.split()
        sample[names[pid]] = {"rss_kib": int(rss), "cpu_s": cpu_seconds(cpu)}
    if lsof:
        sockets = subprocess.run(
            [lsof, "-nP", "-a", "-p", ",".join(names), "-iTCP", "-Fpf"],
            capture_output=True, text=True, check=False,
        )
        name = None
        for row in sockets.stdout.splitlines():
            if row.startswith("p"):
                name = names[row[1:]]
                if name in sample:
                    sample[name]["tcp_sockets"] = 0
            elif row.startswith("f") and name in sample:
                sample[name]["tcp_sockets"] += 1
    return sample


def stop(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def block_connects(stack, count):
    # Fill tiny local listen queues without accepting. Verify that overflow
    # waits for the caller's deadline on this host before starting a trial.
    for port in range(3000, 3000 + count):
        listener = stack.enter_context(socket.socket())
        listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        listener.bind(("127.0.0.1", port))
        listener.listen(1)
        connected = 0
        for _ in range(128):
            client = stack.enter_context(socket.socket())
            client.settimeout(0.2)
            try:
                client.connect(("127.0.0.1", port))
            except TimeoutError:
                if connected:
                    break
                raise RuntimeError(f"port {port} timed out before filling the listen queue")
            connected += 1
        else:
            raise RuntimeError(f"port {port} did not blackhole connects")


def run(args, flags):
    with ExitStack() as stack:
        directory = Path(stack.enter_context(tempfile.TemporaryDirectory(prefix="lb-bench-")))
        processes = {}
        logs = {}
        stats = directory / "backend.jsonl"
        lsof = shutil.which("lsof") if args.sockets else None
        if args.sockets and lsof is None:
            raise RuntimeError("lsof is required for --sockets")

        def launch(name, command, stdout=None, env=None):
            logs[name] = directory / f"{name}.log"
            with logs[name].open("wb") as err, open(stdout or os.devnull, "wb") as out:
                process = subprocess.Popen(command, stdout=out, stderr=err, env=env)
            processes[name] = process
            return process

        try:
            backend_flags = flags
            if args.blackhole:
                block_connects(stack, args.blackhole)
                backend_flags = [*flags, "-refused", str(args.blackhole), "-restore", "0"]
            backend = launch("backend", [args.tool, "-serve", *backend_flags], stats)
            wait_ready(backend, lambda: stats.read_text().startswith("ready\n"), "backend")
            generator = [args.tool, *flags]
            if args.proxy:
                proxy = launch("proxy", [args.proxy], env={**os.environ, "RUST_LOG": args.log})
                wait_ready(proxy, proxy_ready, "proxy")
                generator.append("-proxy")
            results = directory / "generator.jsonl"
            start = time.monotonic()
            load = launch("generator", generator, results)
            samples = []
            while load.poll() is None:
                samples.append(process_stats(processes, start, lsof))
                try:
                    load.wait(timeout=args.sample_interval)
                except subprocess.TimeoutExpired:
                    pass
            if load.returncode:
                raise RuntimeError(f"generator exited with status {load.returncode}")
            for name in ("backend", "proxy"):
                if name in processes and processes[name].poll() is not None:
                    raise RuntimeError(f"{name} exited during the trial")
            samples.append(process_stats(processes, start, lsof))
            if args.cooldown > 0:
                time.sleep(args.cooldown)
                samples.append(process_stats(processes, start, lsof))
            return {
                "proxy": args.proxy,
                "rust_log": args.log,
                "flags": flags,
                "blackholed_backends": args.blackhole,
                "phases": [json.loads(line) for line in results.read_text().splitlines()],
                "process_samples": samples,
                "backend_samples": [
                    json.loads(line) for line in stats.read_text().splitlines()[1:] if line.endswith("}")
                ],
            }
        except (RuntimeError, OSError) as error:
            details = []
            for name, path in logs.items():
                with path.open("rb") as log:
                    log.seek(max(0, path.stat().st_size - 1024))
                    tail = log.read().decode(errors="replace").strip()
                if tail:
                    details.append(f"{name}: {tail}")
            raise SystemExit("\n".join([str(error), *details])) from error
        finally:
            for process in reversed(list(processes.values())):
                stop(process)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tool", required=True, help="path to compiled tcp.go")
    parser.add_argument("--proxy", help="path to proxy release binary, omit for direct runs")
    parser.add_argument("--log", default="error", help="RUST_LOG for the proxy")
    parser.add_argument("--sample-interval", type=float, default=1)
    parser.add_argument("--cooldown", type=float, default=2)
    parser.add_argument("--blackhole", type=int, choices=range(21), default=0,
                        help="fill the listen queues of the first N backend ports")
    parser.add_argument("--sockets", action="store_true", help="sample TCP descriptors with lsof")
    parser.add_argument("flags", nargs=argparse.REMAINDER, help="TCP benchmark flags after --")
    args = parser.parse_args()
    if args.sample_interval <= 0 or args.cooldown < 0:
        parser.error("sample interval must be positive and cooldown nonnegative")
    flags = args.flags[1:] if args.flags[:1] == ["--"] else args.flags
    print(json.dumps(run(args, flags)))


if __name__ == "__main__":
    main()
