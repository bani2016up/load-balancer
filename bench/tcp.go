// A localhost TCP backend and open-loop load generator for the proxy benchmark.
package main

import (
	"bytes"
	"encoding/binary"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"math"
	"math/rand"
	"net"
	"os"
	"sort"
	"strconv"
	"sync"
	"sync/atomic"
	"time"
)

const (
	firstPort = 3000
	backends  = 20
	headerLen = 12
)

var (
	proxy       = flag.Bool("proxy", false, "send traffic through port 8080")
	serve       = flag.Bool("serve", false, "run the benchmark backends")
	rate        = flag.Int("rate", 1000, "scheduled operations per second")
	warmup      = flag.Duration("warmup", 2*time.Second, "warm-up time")
	duration    = flag.Duration("duration", 5*time.Second, "measurement time")
	slots       = flag.Int("concurrency", 512, "maximum outstanding operations or persistent streams")
	deadline    = flag.Duration("deadline", 3*time.Second, "operation deadline including dial")
	persistent  = flag.Bool("persistent", false, "reuse client streams for successive operations")
	size        = flag.Int("bytes", 32, "payload bytes per enabled direction")
	direction   = flag.String("direction", "both", "transfer direction: both, upload, or download")
	delay       = flag.Duration("delay", 0, "delay on every backend operation")
	mixed       = flag.Bool("mixed", false, "seeded 95% fast, 4% 100ms, 1% 1s delay")
	slow        = flag.Int("slow", 0, "number of backends with extra delay")
	slowDelay   = flag.Duration("slow-delay", time.Second, "extra delay on the first -slow backends")
	refused     = flag.Int("refused", 0, "number of backend ports left closed")
	reset       = flag.Int("reset", 0, "number of backends resetting new streams")
	restore     = flag.Duration("restore", 0, "start refused backends after this time")
	idle        = flag.Duration("idle", 0, "wait after dialing before sending")
	readPause   = flag.Duration("read-pause", 0, "pause the client after each 32 KiB read")
	writePause  = flag.Duration("write-pause", 0, "pause the client after each 32 KiB write")
	soakRate    = flag.Int("soak-rate", 0, "overload phase rate after initial measurement")
	soak        = flag.Duration("soak", 30*time.Second, "overload phase length")
	recovery    = flag.Duration("recovery", 10*time.Second, "recovery phase length at initial rate")
	p99Budget   = flag.Duration("p99-budget", 25*time.Millisecond, "scheduled p99 latency budget")
	errorBudget = flag.Float64("error-budget", 0.001, "maximum fraction of unsuccessful operations")
)

type backendStats struct {
	Accepted atomic.Uint64
	Active   atomic.Int64
	Peak     atomic.Int64
}

func address(port int) string { return "127.0.0.1:" + strconv.Itoa(port) }

func listen(index int, stats *backendStats, payload []byte) {
	port := firstPort + index
	listener, err := net.Listen("tcp4", address(port))
	if err != nil {
		fatal(err)
	}
	go func() {
		for {
			conn, err := listener.Accept()
			if err != nil {
				fatal(err)
			}
			stats.Accepted.Add(1)
			active := stats.Active.Add(1)
			for peak := stats.Peak.Load(); active > peak; peak = stats.Peak.Load() {
				if stats.Peak.CompareAndSwap(peak, active) {
					break
				}
			}
			go func() {
				defer stats.Active.Add(-1)
				defer conn.Close()
				if index >= *refused && index < *refused+*reset {
					conn.(*net.TCPConn).SetLinger(0)
					return
				}
				relay(conn, port, index < *slow, payload)
			}()
		}
	}()
}

func relay(conn net.Conn, port int, slowBackend bool, payload []byte) {
	var header [headerLen]byte
	var body []byte
	for {
		if _, err := io.ReadFull(conn, header[:]); err != nil {
			return
		}
		length := int(binary.BigEndian.Uint32(header[4:8]))
		if length > *size {
			return
		}
		wait := *delay + time.Duration(binary.BigEndian.Uint16(header[8:10]))*time.Millisecond
		if slowBackend {
			wait += *slowDelay
		}
		if wait > 0 {
			time.Sleep(wait)
		}
		binary.BigEndian.PutUint16(header[10:12], uint16(port))
		if *direction != "both" {
			if !transferOneWay(conn, header, length, payload, &body) {
				return
			}
			continue
		}
		if length <= 1024 {
			var packet [headerLen + 1024]byte
			copy(packet[:], header[:])
			if _, err := io.ReadFull(conn, packet[headerLen:headerLen+length]); err != nil {
				return
			}
			if _, err := conn.Write(packet[:headerLen+length]); err != nil {
				return
			}
			continue
		}
		if len(body) < min(length, 32768) {
			body = make([]byte, min(length, 32768))
		}
		if _, err := conn.Write(header[:]); err != nil {
			return
		}
		for remaining := length; remaining > 0; {
			chunk := body[:min(remaining, len(body))]
			if _, err := io.ReadFull(conn, chunk); err != nil {
				return
			}
			if _, err := conn.Write(chunk); err != nil {
				return
			}
			remaining -= len(chunk)
		}
	}
}

func transferOneWay(conn net.Conn, header [headerLen]byte, length int, payload []byte, body *[]byte) bool {
	if *direction == "upload" {
		if len(*body) < min(length, len(payload)) {
			*body = make([]byte, min(length, len(payload)))
		}
		for remaining := length; remaining > 0; {
			chunk := (*body)[:min(remaining, len(*body))]
			if _, err := io.ReadFull(conn, chunk); err != nil || !bytes.Equal(chunk, payload[:len(chunk)]) {
				return false
			}
			remaining -= len(chunk)
		}
		_, err := conn.Write(header[:])
		return err == nil
	}
	if _, err := conn.Write(header[:]); err != nil {
		return false
	}
	for remaining := length; remaining > 0; {
		chunk := payload[:min(remaining, len(payload))]
		if _, err := conn.Write(chunk); err != nil {
			return false
		}
		remaining -= len(chunk)
	}
	return true
}

func server(payload []byte) {
	var stats [backends]backendStats
	for i := *refused; i < backends; i++ {
		listen(i, &stats[i], payload)
	}
	fmt.Println("ready")
	if *restore > 0 {
		time.AfterFunc(*restore, func() {
			for i := 0; i < *refused; i++ {
				listen(i, &stats[i], payload)
			}
		})
	}
	for range time.Tick(time.Second) {
		snapshot := make(map[int][3]uint64, backends)
		for i := range stats {
			snapshot[firstPort+i] = [3]uint64{
				stats[i].Accepted.Load(), uint64(stats[i].Active.Load()), uint64(stats[i].Peak.Load()),
			}
		}
		output(snapshot)
	}
}

type client struct {
	conn    net.Conn
	backend int
}

func (c *client) close() {
	if c.conn != nil {
		c.conn.Close()
		c.conn = nil
	}
}

func (c *client) exchange(port int, id uint32, delayMs uint16, scheduled time.Time, payload []byte) (int, bool, error) {
	newConn := c.conn == nil
	if newConn {
		dialer := net.Dialer{Deadline: scheduled.Add(*deadline)}
		conn, err := dialer.Dial("tcp4", address(port))
		if err != nil {
			return 0, false, err
		}
		c.conn = conn
	}
	c.conn.SetDeadline(scheduled.Add(*deadline))
	if *idle > 0 {
		time.Sleep(*idle)
	}
	var header [headerLen]byte
	binary.BigEndian.PutUint32(header[:4], id)
	binary.BigEndian.PutUint32(header[4:8], uint32(*size))
	binary.BigEndian.PutUint16(header[8:10], delayMs)
	if *direction == "download" {
		if _, err := c.conn.Write(header[:]); err != nil {
			return 0, newConn, err
		}
		return c.receive(header, port, newConn, payload)
	}
	if *size <= 1024 && *writePause == 0 {
		var packet [headerLen + 1024]byte
		copy(packet[:], header[:])
		copy(packet[headerLen:], payload)
		if _, err := c.conn.Write(packet[:headerLen+*size]); err != nil {
			return 0, newConn, err
		}
		return c.receive(header, port, newConn, payload)
	}

	// Drain the echo while writing. Either direction can block on backpressure.
	conn := c.conn
	written := make(chan error, 1)
	go func() {
		_, err := conn.Write(header[:])
		for remaining := *size; err == nil && remaining > 0; {
			chunk := payload[:min(remaining, len(payload))]
			_, err = conn.Write(chunk)
			remaining -= len(chunk)
			if *writePause > 0 && remaining > 0 {
				time.Sleep(*writePause)
			}
		}
		written <- err
	}()
	backend, _, err := c.receive(header, port, newConn, payload)
	if err != nil {
		conn.Close()
	}
	writeErr := <-written
	if err == nil {
		err = writeErr
	}
	return backend, newConn, err
}

var invalidResponse = errors.New("invalid response")

func (c *client) receive(header [headerLen]byte, port int, newConn bool, payload []byte) (int, bool, error) {
	var received [headerLen]byte
	if _, err := io.ReadFull(c.conn, received[:]); err != nil {
		return 0, newConn, err
	}
	backend := int(binary.BigEndian.Uint16(received[10:12]))
	if !bytes.Equal(received[:10], header[:10]) || backend < firstPort || backend >= firstPort+backends ||
		(newConn && !*proxy && port != backend) || (!newConn && backend != c.backend) {
		return 0, newConn, invalidResponse
	}
	if *direction != "upload" {
		if err := c.verifyPayload(payload); err != nil {
			return 0, newConn, err
		}
	}
	c.backend = backend
	return backend, newConn, nil
}

func (c *client) verifyPayload(payload []byte) error {
	// The small buffer keeps tiny requests from allocating streaming-sized buffers.
	var small [1024]byte
	buffer := small[:]
	if *size > len(small) {
		buffer = make([]byte, len(payload))
	}
	for remaining := *size; remaining > 0; {
		chunk := buffer[:min(remaining, len(buffer))]
		if _, err := io.ReadFull(c.conn, chunk); err != nil {
			return err
		}
		if !bytes.Equal(chunk, payload[:len(chunk)]) {
			return invalidResponse
		}
		remaining -= len(chunk)
		if *readPause > 0 && remaining > 0 {
			time.Sleep(*readPause)
		}
	}
	return nil
}

type result struct {
	latency time.Duration
	late    time.Duration
	backend int
	newConn bool
	failure string
}

type phase struct {
	name     string
	duration time.Duration
	rate     int
	results  []result
}

func (p *phase) schedule(start time.Time, clients chan *client, pending *sync.WaitGroup, firstID uint32, payload []byte) uint32 {
	rng := rand.New(rand.NewSource(42))
	for i := range p.results {
		scheduled := start.Add(time.Duration(i) * time.Second / time.Duration(p.rate))
		if wait := time.Until(scheduled); wait > 0 {
			time.Sleep(wait)
		}
		var delayMs uint16
		if *mixed {
			draw := rng.Intn(100)
			switch {
			case draw == 99:
				delayMs = 1000
			case draw >= 95:
				delayMs = 100
			}
		}
		res := &p.results[i]
		res.late = time.Since(scheduled)
		if res.late >= *deadline {
			res.failure = "scheduler_deadline"
			continue
		}
		var c *client
		select {
		case c = <-clients:
		default:
			res.failure = "generator_limit"
			continue
		}
		port := 8080
		if !*proxy {
			port = firstPort + i%backends
		}
		pending.Add(1)
		go func() {
			defer pending.Done()
			backend, newConn, err := c.exchange(port, firstID+uint32(i), delayMs, scheduled, payload)
			res.latency, res.backend, res.newConn = time.Since(scheduled), backend, newConn
			var netErr net.Error
			switch {
			case errors.Is(err, invalidResponse):
				res.failure = "corrupt_response"
			case errors.As(err, &netErr) && netErr.Timeout():
				res.failure = "timeout"
			case err != nil:
				res.failure = err.Error()
				// Socket addresses in errors can make every error string unique.
				if opErr, ok := err.(*net.OpError); ok {
					res.failure = opErr.Op + ": " + opErr.Err.Error()
				}
			}
			if err != nil || !*persistent {
				c.close()
			}
			clients <- c
		}()
	}
	return firstID + uint32(len(p.results))
}

func report(name string, duration time.Duration, rate int, results []result) {
	var latencies, lateness []time.Duration
	backends := make(map[int]int)
	errors := make(map[string]int)
	connections := 0
	for _, result := range results {
		lateness = append(lateness, result.late)
		if result.newConn {
			connections++
		}
		if result.failure != "" {
			errors[result.failure]++
			continue
		}
		latencies = append(latencies, result.latency)
		backends[result.backend]++
	}
	sort.Slice(latencies, func(i, j int) bool { return latencies[i] < latencies[j] })
	sort.Slice(lateness, func(i, j int) bool { return lateness[i] < lateness[j] })
	successes := len(latencies)
	bytes := float64(successes) * float64(*size)
	if *direction == "both" {
		bytes *= 2
	}
	output(map[string]any{
		"phase": name, "rate": rate, "seconds": duration.Seconds(), "scheduled": len(results),
		"successful": successes, "operations_per_s": float64(successes) / duration.Seconds(),
		"connections_per_s":   float64(connections) / duration.Seconds(),
		"payload_bytes_per_s": bytes / duration.Seconds(),
		"p50_ms":              percentile(latencies, .5), "p95_ms": percentile(latencies, .95),
		"p99_ms": percentile(latencies, .99), "p999_ms": percentile(latencies, .999),
		"scheduler_p99_ms": percentile(lateness, .99), "successful_by_backend": backends, "errors": errors,
		"meets_budget": successes > 0 && errors["corrupt_response"] == 0 &&
			percentile(latencies, .99) <= float64(*p99Budget)/float64(time.Millisecond) &&
			float64(len(results)-successes)/float64(len(results)) <= *errorBudget,
	})
}

func percentile(sorted []time.Duration, fraction float64) float64 {
	if len(sorted) == 0 {
		return 0
	}
	index := int(math.Ceil(float64(len(sorted))*fraction)) - 1
	return float64(sorted[index]) / float64(time.Millisecond)
}

func output(data any) {
	if err := json.NewEncoder(os.Stdout).Encode(data); err != nil {
		fatal(err)
	}
}

func fatal(err error) {
	fmt.Fprintln(os.Stderr, err)
	os.Exit(1)
}

func main() {
	flag.Parse()
	if *rate <= 0 || *slots <= 0 || *size < 0 || uint64(*size) > math.MaxUint32 || *duration <= 0 ||
		*deadline <= 0 || *idle >= *deadline || *refused < 0 || *reset < 0 || *refused+*reset > backends ||
		*slow < 0 || *slow > backends || *soakRate < 0 || *warmup < 0 || *soak < 0 || *recovery < 0 || *p99Budget <= 0 || *errorBudget < 0 || *errorBudget > 1 {
		fatal(errors.New("invalid benchmark settings"))
	}
	if *direction != "both" && *direction != "upload" && *direction != "download" {
		fatal(errors.New("invalid transfer direction"))
	}
	payload := make([]byte, min(*size, 32768))
	rand.New(rand.NewSource(42)).Read(payload)
	if *serve {
		server(payload)
		return
	}
	clients := make(chan *client, *slots)
	for range *slots {
		clients <- &client{}
	}
	phases := []phase{{name: "warmup", duration: *warmup, rate: *rate},
		{name: "measure", duration: *duration, rate: *rate}}
	if *soakRate > 0 {
		phases = append(phases, phase{name: "overload", duration: *soak, rate: *soakRate},
			phase{name: "recovery", duration: *recovery, rate: *rate})
	}
	for i := range phases {
		p := &phases[i]
		p.results = make([]result, int(p.duration.Seconds()*float64(p.rate)))
	}
	var pending sync.WaitGroup
	var id uint32
	start := time.Now()
	for i := range phases {
		p := &phases[i]
		id = p.schedule(start, clients, &pending, id, payload)
		start = start.Add(p.duration)
	}
	pending.Wait()
	for _, p := range phases {
		if len(p.results) > 0 {
			report(p.name, p.duration, p.rate, p.results)
		}
	}
	for range *slots {
		(<-clients).close()
	}
}
