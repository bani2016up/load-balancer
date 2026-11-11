# TCP proxy benchmark

Build the Go 1.22+ benchmark tool and the Rust release binary from the repository root. The runner uses Python 3.9+ and localhost ports 8080 and 3000–3019. Free those ports before starting.

```sh
go build -o /tmp/tcp-bench bench/tcp.go
cargo build --locked --release
python3 bench/matrix.py --tool /tmp/tcp-bench --direct \
  --proxy ./target/release/load-balancer --sockets --out /tmp/trials.jsonl
```

Pass `--match 'churn|persistent'` to select workloads. Pass multiple `--proxy` paths to compare builds in the same seeded trial order. Each trial starts fresh processes and uses the same 20 backend ports in direct and proxy modes. The runner appends JSON Lines to the output file.

Run an individual workload to adjust the offered rate, concurrency, deadlines, or payload size. The arguments after `--` go to the benchmark tool.

```sh
python3 bench/run.py --tool /tmp/tcp-bench \
  --proxy ./target/release/load-balancer --log error --sockets -- \
  -rate 4000 -concurrency 1024 -warmup 3s -duration 10s -p99-budget 10ms
/tmp/tcp-bench -help
```

## Measurement

The driver schedules arrivals independently of completion. Latency starts at the scheduled arrival, so dispatch delays count. A full driver concurrency window produces a `generator_limit` failure. Raise the window and check driver CPU and scheduler lateness before attributing a limit to the proxy.

An operation succeeds when the client verifies the response ID, backend identity, and all expected bytes. The upload-only backend acknowledges after verifying the payload. Persistent operations also verify the backend stays the same. Connection rates count completed client dials, including those followed by failed operations. For connection churn, the backend sends the first FIN and the client verifies EOF. This reduces driver port exhaustion during repeated local runs. TCP closes and resets are reported as observed by the client.

The runner records each phase's throughput, success latency percentiles, failures, and distribution. Sampled process RSS and CPU time include the backend, driver, and proxy. With `--sockets`, samples also include open TCP descriptors, including listeners. Per-backend samples list accepted, active, and peak active connections in that order. RSS excludes kernel socket memory.

The matrix declares a p99 latency budget for each workload and a default failure budget of 0.1%. Treat a rate as sustainable only when repetitions meet both budgets and resource use stabilizes. Use the highest passing *tested* rate. Extend the sweep if the last rate passes or if the driver limits the result.

The overload case transitions from warmup to measurement, overload, and recovery on one timeline. Compare RSS and socket counts during each phase and after cooldown. The blackhole case verifies that new dials time out against a bound socket or a full listen queue before measuring. It depends on the host's TCP behavior.

Keep the host otherwise idle and report trial variation. Direct and proxied traffic share the host's CPU and TCP stack with the driver. High-rate connection churn can exhaust the local ephemeral port range. A local saturation point includes these effects.
