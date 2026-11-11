# TCP performance results

Load-aware backend selection made the largest measured latency difference. With four slow backends, it cut p99 from about 1,006 ms to 26–30 ms at 3,000 verified operations/s. Healthy churn at the same rate had higher p99 after the changes. Admission control bounded open connections through a sustained overload.

The original build was `26de78a`. The optimized build uses the application code at `3bcab43`. The [benchmark guide](README.md) describes the runner and its raw JSON output.

## Method

Trials ran on one Apple M2 Pro with 10 CPUs, 16 GiB of RAM, and macOS 27.2. Builds used Rust 1.99.1 and Go 1.27.3. The driver, proxy, and 20 backend listeners shared the host and its TCP stack.

The reference sweep used 1 second of warmup and 4 seconds of scheduled arrivals per configuration. The final sweep used 2 seconds of warmup and 5 seconds of arrivals. Overload ran for 35 seconds before an 8-second recovery phase. The long-lived case kept traffic flowing across the 30-second relay deadline. The main sweep suppressed success logs and enabled admission rejection logs on the optimized build.

Each row reports verified operations per second and p99 latency in milliseconds. Ranges show trial variation. A pass requires the workload's predeclared p99 budget and at most 0.1% failed operations. Driver limits and dial failures count against the error budget. A short-trial pass establishes a tested load point. Resource stability also matters for sustained load.

## Full workload sweep

| Workload | Offered/s | Direct | Original | Optimized | Optimized passes |
| --- | ---: | ---: | ---: | ---: | ---: |
| bidir | 100 | 100 / 14.3–15.3 | 100 / 16.7 | 100 / 31.4–33.6 | 2/2 |
| bidir | 400 | 140–400 / 53.7–2,837.7 | 5 / 2,942.6 | 0–7 / 2,989.2 | 0/2 |
| blackhole | 1,000 | 950 / 6.1–28.4 | 950 / 22.5 | 1,000 / 6.0–33.6 | 1/2 |
| churn | 1,000 | 1,000 / 9.3–14.9 | 1,000 / 8.8 | 1,000 / 12.7–18.3 | 2/2 |
| churn | 4,000 | 4,000 / 6.1–11.2 | 4,000 / 25.5 | 4,000 / 31.7–51.2 | 0/2 |
| churn | 8,000 | 8,000 / 44.1–56.4 | 6,710 / 215.1 | 5,032–6,050 / 283.9–468.4 | 0/2 |
| churn | 12,000 | 12,000 / 31.9–42.3 | 6,546 / 506.9 | 3,290–6,205 / 456.2–1,184.4 | 0/2 |
| delay10 | 1,000 | 1,000 / 27.2–42.8 | 1,000 / 71.6 | 1,000 / 34.4–40.5 | 0/2 |
| delay10 | 4,000 | 4,000 / 36.7–73.1 | 4,000 / 42.7 | 4,000 / 55.3–73.0 | 0/2 |
| delay100 | 1,000 | 1,000 / 114.3–119.8 | 1,000 / 109.2 | 1,000 / 124.2–159.6 | 1/2 |
| delay100 | 4,000 | 4,000 / 143.7–167.8 | 4,000 / 149.9 | 4,000 / 140.9–156.0 | 0/2 |
| delay1000 | 250 | 250 / 1,036.4–1,067.0 | 250 / 1,007.2 | 250 / 1,016.2–1,038.4 | 2/2 |
| delay1000 | 1,000 | 1,000 / 1,023.0–1,030.0 | 1,000 / 1,014.3 | 1,000 / 1,015.3–1,036.9 | 2/2 |
| download | 100 | 100 / 17.1–23.2 | 100 / 9.7 | 100 / 19.1–83.5 | 2/2 |
| idle | 250 | 250 / 2,034.5–2,040.0 | 250 / 2,014.2 | 250 / 2,018.6–2,035.0 | 2/2 |
| long-lived | 1,000 | 1,000 / 3.1–6.5 | 1,000 / 18.6 | 1,000 / 2.5–4.4 | 2/2 |
| mixed | 1,000 | 1,000 / 260.5–1,000.5 | 1,000 / 1,000.5 | 1,000 / 108.8–181.4 | 2/2 |
| mixed | 4,000 | 4,000 / 1,000.1–1,000.2 | 4,000 / 1,000.3 | 4,000 / 1,000.2 | 2/2 |
| overload | 300 | 300 / 1,008.9–1,041.4 | 300 / 1,006.6 | 300 / 1,010.8–1,046.3 | 2/2 |
| persistent | 1,000 | 1,000 / 1.4–1.8 | 1,000 / 5.4 | 1,000 / 2.4–10.3 | 2/2 |
| persistent | 4,000 | 3,986–4,000 / 0.2–9.2 | 4,000 / 0.8 | 3,892–3,910 / 31.2–35.6 | 0/2 |
| persistent | 8,000 | 8,000 / 1.9–7.3 | 8,000 / 5.4 | 8,000 / 3.4–10.4 | 2/2 |
| refused | 1,000 | 950 / 4.7–11.3 | 950 / 15.2 | 1,000 / 6.3–29.7 | 1/2 |
| reset | 1,000 | 950 / 8.7–28.8 | 950 / 28.8 | 949–950 / 15.6–33.6 | 0/2 |
| restore | 1,000 | 950–970 / 5.1–8.2 | 951 / 17.2 | 1,000 / 25.9–43.6 | 0/2 |
| slow-one | 1,000 | 1,000 / 1,001.6–1,002.5 | 1,000 / 1,001.8 | 1,000 / 18.8–23.0 | 2/2 |
| slow-reader | 50 | 50 / 351.4–383.2 | 50 / 352.0 | 50 / 371.7–374.7 | 2/2 |
| slow-several | 4,000 | 4,000 / 1,001.1–1,001.9 | 3,934 / 1,373.8 | 4,000 / 53.5–1,008.1 | 1/2 |
| slow-writer | 50 | 50 / 359.8–387.8 | 50 / 392.2 | 50 / 372.1–377.2 | 2/2 |
| upload | 100 | 100 / 19.2–29.6 | 100 / 14.1 | 100 / 8.6–17.7 | 2/2 |

The table uses the measurement phase for every workload. Latency comes from successful operations. At 400 bidirectional arrivals/s, one optimized run had zero successes, leaving one p99 value.

At 4,000 operations/s with four slow backends, the optimized runs sent 119 and 201 of 20,000 operations to slow backends. That difference put the one-second delay beyond p99 in one run and inside p99 in the other.

The long-lived case recorded 64 failures in each optimized run as its 64 streams crossed the relay deadline. The restored backend served traffic again in both recovery trials.

## Additional rate and concurrency checks

These trials used 2 seconds of warmup and 4 seconds of arrivals. The payloads and injected faults match the corresponding workload in the main sweep.

| Workload | Offered/s | Driver slots | Direct | Optimized | Optimized passes |
| --- | ---: | ---: | ---: | ---: | ---: |
| bidir | 175 | 128 | 175 / 9.5–20.2 | 175 / 23.6–45.9 | 2/2 |
| blackhole | 2,000 | 1,024 | 1,900 / 10.4–15.8 | 1,976–2,000 / 14.8–144.3 | 1/2 |
| download | 175 | 128 | 175 / 12.8–19.1 | 175 / 8.1–12.3 | 2/2 |
| idle | 750 | 2,048 | 750 / 2,014.2–2,025.1 | 750 / 2,018.0–2,024.5 | 2/2 |
| persistent | 4,000 | 512 | 4,000 / 1.4–8.7 | 4,000 / 0.6–1.7 | 2/2 |
| refused | 2,000 | 1,024 | 1,900 / 12.0–14.3 | 2,000 / 7.8–109.5 | 1/2 |
| reset | 2,000 | 1,024 | 1,900 / 7.6–8.0 | 1,915–1,968 / 10.6–14.6 | 0/2 |
| slow-one | 2,000 | 1,024 | 2,000 / 1,001.7–1,001.8 | 2,000 / 6.0–12.0 | 2/2 |
| slow-reader | 100 | 128 | 100 / 374.6–380.1 | 100 / 375.5–379.0 | 2/2 |
| slow-writer | 100 | 128 | 100 / 415.4–613.8 | 100 / 358.8–359.8 | 2/2 |
| upload | 175 | 128 | 175 / 7.2–9.2 | 175 / 13.3–14.1 | 2/2 |

Both repeats met their budgets at 2,000 healthy churn operations/s, 8,000 persistent operations/s with 512 driver slots, and 175 bidirectional operations/s. Those are tested load points on this host.

## Overload and recovery

Each matched soak offered 300 operations/s, raised arrivals to 3,000/s for 35 seconds, then returned to 300/s for 8 seconds. The backend delay was one second.

| Measurement | Original | Optimized with the default cap |
| --- | ---: | ---: |
| Verified operations/s during overload | 2,999–3,000 | 2,017–2,018 |
| Overload p99 | 1,070–1,234 ms | 1,052–1,065 ms |
| Failed operations during overload | 0–47 | 34,384–34,402 |
| Proxy admission rejection log lines | 0 | 34,173–34,402 |
| Peak open proxy TCP descriptors | 6,229–6,309 | 4,097 |
| Peak proxy RSS | 55.5–69.0 MiB | 37.2 MiB |
| Verified operations/s during recovery | 300 | 299–300 |
| Failed operations during recovery | 0 | 0–8 |
| Open proxy TCP descriptors after cooldown | 1 | 1 |
| RSS after one second of cooldown | 7.9–25.9 MiB | 32.9–37.1 MiB |

The default cap traded admitted throughput for a lower peak descriptor and memory budget. During the middle of overload, optimized RSS stayed between 37.0 and 37.2 MiB. The remaining TCP descriptor after cooldown was the listener. Process RSS stayed elevated after the other descriptors closed.

With `LOAD_BALANCER_MAX_CONNECTIONS=4096`, the same 3,000/s phase verified 2,976–3,000 operations/s. Peak TCP descriptors were 6,155–6,245 and peak RSS was 56.8–61.7 MiB. The overload p99 was 1,071.3–1,200.7 ms.

In a third default-cap soak, peak RSS was 37.2 MiB. After 30 seconds of cooldown it was 1.5 MiB, with one TCP descriptor open.

## Tuning comparisons

- At 3,000 operations/s with four slow backends, load-aware selection cut p99 from 1,006–1,007 ms to 26–30 ms. Peak proxy RSS fell from 13.6–14.1 MiB to 3.6–3.7 MiB. Both variants verified every response.
- With one refused or blackholed backend, the routing change raised verified throughput from 950 to 1,000 operations/s in both paired runs. Retries occur during connect, before the proxy forwards data.
- At 2,000 healthy churn operations/s, the optimized build passed both 25 ms p99 trials. At 3,000 operations/s, original p99 was 8.5–18.3 ms and optimized p99 was 25.6–34.8 ms. Both delivered all 3,000 operations/s. Scheduler lateness was higher in the optimized runs. Reusing the accept loop's sequence number for backend selection produced overlapping latency and CPU ranges across three paired trials.
- Suppressing per-connection success logs reduced sampled proxy CPU time from 2.65–2.81 to 2.48–2.55 seconds at 4,000 offered connections/s. The lower-logging runs included one trial with 51 failures. Completion timing and byte counts are available at trace level.
- Startup socket addresses removed address formatting and parsing from each dial. The isolated comparison delivered 4,000 operations/s in all four trials, with overlapping p99 ranges and 2.64–2.65 versus 2.68–2.70 proxy CPU seconds.
- Forcing `TCP_NODELAY` had overlapping persistent latency ranges. Two Tokio workers lowered p99 at 4,000 churn operations/s in the tuning trials. The lower-rate repeats did not reproduce a consistent advantage. The proxy uses Tokio's default worker count.
- At 750 two-second idle arrivals/s, peak proxy RSS was 16–17 MiB with 4 KiB buffers per direction, 26–28 MiB with 8 KiB, and 76–100 MiB with 32 KiB. In the 100-operation/s bidirectional test, 4 KiB buffers raised sampled proxy CPU from 3.14–3.35 to 5.34–5.40 seconds. The proxy uses the 8 KiB Tokio relay buffers.
- In two 175-operation/s, 4 MiB bidirectional trials, default socket buffers verified every transfer at 59–63 ms p99. The 128 KiB socket buffer experiment verified 14–17 operations/s. The proxy uses system socket buffer settings.

In a separate bulk profile, `sendto` and `recvfrom` were the most frequent sampled socket calls. At 250 offered bidirectional transfers/s, host network buffers reached 98.9% use and the kernel reported allocation denials. That profile included driver socket failures and timeouts. The default loopback send and receive buffer capacities were 146,988 and 408,300 bytes per socket. Kernel counters are host-wide. Proxy RSS does not include kernel socket memory.

## Verification

All 14 Rust tests passed with a local listener for the existing session-affinity test on port 8080. The release build, formatting checks for changed Rust files, Go build and vet, and Python syntax checks passed.

The 60 optimized runs in the main sweep logged zero panics and had zero corrupt responses. The original refusal trial logged 351 task panics across readiness, warmup, and measurement.
