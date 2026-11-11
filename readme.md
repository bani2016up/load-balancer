# TCP Load Balancer

A Tokio TCP proxy that listens on `127.0.0.1:8080` and forwards to 20 local backends on ports `3000`–`3019`. It chooses a backend per connection. Persistent connections stay on their original backend.

## Quick start

Start the backends, then build and run the proxy.

```sh
cargo build --locked --release
./target/release/load-balancer
```

## Connection handling

The proxy prefers the available backend with the fewest active connections, including dials in progress. It rotates among ties. A failed backend connect can trigger one retry before relay begins. Each dial has a 250 ms deadline. Failed backend dials pause selection of that backend for about one second, then one new connection probes it.

The default cap is 2,048 active client connections, including those connecting to a backend. At capacity, the proxy closes newly accepted sockets. Set `LOAD_BALANCER_MAX_CONNECTIONS` to change the cap. Budget up to two TCP descriptors per admitted connection, plus the listener and other process descriptors.

Each admitted client uses a dedicated upstream TCP connection. The proxy forwards bytes and TCP half-closes in both directions. The 30-second relay deadline starts when the backend connects and applies even while data is flowing.

Set `RUST_LOG=info,load_balancer::core::load_balancer=trace` to log completed connections. Use `RUST_LOG=info,load_balancer::admission=debug` for admission rejections.

## Benchmarks

The [benchmark guide](bench/README.md) covers direct and proxied runs for connection churn, persistent traffic, delayed and failing backends, large transfers, and overload recovery.

## License

MIT
