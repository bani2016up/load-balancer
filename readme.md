# Load Balancer

High-performance Rust load balancer with round-robin distribution and session affinity.

# Quick Start

## Build

```bash
cargo build --release
```

## Run

```bash
./target/release/load-balancer
```

# Features

* Round-robin load balancing
* Session affinity
* Connection pooling
* Configurable timeouts
* 7,500+ RPS performance

---

# Test with Apache Bench

## Start python test servers

```bash
cd test_server
for port in {3000..3019}; do
    uvicorn --host 127.0.0.1 --port $port --workers 1 main:app &
    echo "Started backend on port $port"
    sleep 0.1
done

echo "Backends started. Press Ctrl+C to stop."
wait
```

## Run LB

```bash
./target/release/load-balancer
```

## Start AB setst

```bash
ab -n 10000 -c 100 http://127.0.0.1:8080/
```

---

# License

## MIT
