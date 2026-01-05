//! Transport metrics and observability.

use crate::mux::ChannelType;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Simple counter
struct Counter {
    value: AtomicU64,
}

impl Counter {
    fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
        }
    }

    fn inc(&self, by: u64) {
        self.value.fetch_add(by, Ordering::Relaxed);
    }

    fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

/// Simple histogram for RTT tracking
struct Histogram {
    buckets: Mutex<Vec<(f64, u64)>>, // (upper_bound, count)
    sum: AtomicU64,
    count: AtomicU64,
}

impl Histogram {
    fn new() -> Self {
        // Create buckets: 0-10ms, 10-50ms, 50-100ms, 100-500ms, 500ms+
        let buckets = vec![
            (0.010, 0),  // 10ms
            (0.050, 0),  // 50ms
            (0.100, 0),  // 100ms
            (0.500, 0),  // 500ms
            (f64::INFINITY, 0),
        ];
        Self {
            buckets: Mutex::new(buckets),
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    fn record(&self, value: Duration) {
        let secs = value.as_secs_f64();
        self.sum.fetch_add(value.as_millis() as u64, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);

        let mut buckets = self.buckets.lock();
        for (upper_bound, count) in buckets.iter_mut() {
            if secs <= *upper_bound {
                *count += 1;
                break;
            }
        }
    }

    fn get(&self) -> (u64, u64, Vec<(f64, u64)>) {
        let buckets = self.buckets.lock().clone();
        (
            self.sum.load(Ordering::Relaxed),
            self.count.load(Ordering::Relaxed),
            buckets,
        )
    }

    fn reset(&self) {
        let mut buckets = self.buckets.lock();
        for (_, count) in buckets.iter_mut() {
            *count = 0;
        }
        self.sum.store(0, Ordering::Relaxed);
        self.count.store(0, Ordering::Relaxed);
    }
}

/// Transport metrics tracker
pub struct TransportMetrics {
    prefix: String,
    bytes_sent: Counter,
    bytes_received: Counter,
    messages_sent: Counter,
    messages_received: Counter,
    frames_dropped: Counter,
    rtt_histogram: Histogram,
    connection_duration: Histogram,
    channel_bytes_sent: Mutex<HashMap<ChannelType, Counter>>,
    channel_bytes_received: Mutex<HashMap<ChannelType, Counter>>,
    channel_dropped: Mutex<HashMap<ChannelType, Counter>>,
}

impl TransportMetrics {
    /// Create new metrics with prefix
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
            bytes_sent: Counter::new(),
            bytes_received: Counter::new(),
            messages_sent: Counter::new(),
            messages_received: Counter::new(),
            frames_dropped: Counter::new(),
            rtt_histogram: Histogram::new(),
            connection_duration: Histogram::new(),
            channel_bytes_sent: Mutex::new(HashMap::new()),
            channel_bytes_received: Mutex::new(HashMap::new()),
            channel_dropped: Mutex::new(HashMap::new()),
        }
    }

    /// Record bytes sent
    pub fn record_send(&self, channel: ChannelType, bytes: usize) {
        self.bytes_sent.inc(bytes as u64);
        self.messages_sent.inc(1);
        
        let mut map = self.channel_bytes_sent.lock();
        map.entry(channel)
            .or_insert_with(Counter::new)
            .inc(bytes as u64);
    }

    /// Record bytes received
    pub fn record_recv(&self, channel: ChannelType, bytes: usize) {
        self.bytes_received.inc(bytes as u64);
        self.messages_received.inc(1);
        
        let mut map = self.channel_bytes_received.lock();
        map.entry(channel)
            .or_insert_with(Counter::new)
            .inc(bytes as u64);
    }

    /// Record dropped frame
    pub fn record_drop(&self, channel: ChannelType) {
        self.frames_dropped.inc(1);
        
        let mut map = self.channel_dropped.lock();
        map.entry(channel)
            .or_insert_with(Counter::new)
            .inc(1);
    }

    /// Record RTT measurement
    pub fn record_rtt(&self, rtt: Duration) {
        self.rtt_histogram.record(rtt);
    }

    /// Record connection duration
    pub fn record_connection_duration(&self, duration: Duration) {
        self.connection_duration.record(duration);
    }

    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();
        let prefix = &self.prefix;

        // Global counters
        output.push_str(&format!(
            "# HELP {}_bytes_sent_total Total bytes sent\n",
            prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_bytes_sent_total counter\n",
            prefix
        ));
        output.push_str(&format!(
            "{}_bytes_sent_total {}\n",
            prefix,
            self.bytes_sent.get()
        ));

        output.push_str(&format!(
            "# HELP {}_bytes_received_total Total bytes received\n",
            prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_bytes_received_total counter\n",
            prefix
        ));
        output.push_str(&format!(
            "{}_bytes_received_total {}\n",
            prefix,
            self.bytes_received.get()
        ));

        output.push_str(&format!(
            "# HELP {}_messages_sent_total Total messages sent\n",
            prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_messages_sent_total counter\n",
            prefix
        ));
        output.push_str(&format!(
            "{}_messages_sent_total {}\n",
            prefix,
            self.messages_sent.get()
        ));

        output.push_str(&format!(
            "# HELP {}_messages_received_total Total messages received\n",
            prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_messages_received_total counter\n",
            prefix
        ));
        output.push_str(&format!(
            "{}_messages_received_total {}\n",
            prefix,
            self.messages_received.get()
        ));

        output.push_str(&format!(
            "# HELP {}_frames_dropped_total Total frames dropped\n",
            prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_frames_dropped_total counter\n",
            prefix
        ));
        output.push_str(&format!(
            "{}_frames_dropped_total {}\n",
            prefix,
            self.frames_dropped.get()
        ));

        // RTT histogram
        let (sum, count, _buckets) = self.rtt_histogram.get();
        if count > 0 {
            output.push_str(&format!(
                "# HELP {}_rtt_milliseconds Round-trip time in milliseconds\n",
                prefix
            ));
            output.push_str(&format!(
                "# TYPE {}_rtt_milliseconds histogram\n",
                prefix
            ));
            let avg = sum / count;
            output.push_str(&format!("{}_rtt_milliseconds_sum {}\n", prefix, sum));
            output.push_str(&format!("{}_rtt_milliseconds_count {}\n", prefix, count));
            output.push_str(&format!("{}_rtt_milliseconds_avg {}\n", prefix, avg));
        }

        output
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.bytes_sent.reset();
        self.bytes_received.reset();
        self.messages_sent.reset();
        self.messages_received.reset();
        self.frames_dropped.reset();
        self.rtt_histogram.reset();
        self.connection_duration.reset();
        
        let mut sent_map = self.channel_bytes_sent.lock();
        sent_map.clear();
        let mut recv_map = self.channel_bytes_received.lock();
        recv_map.clear();
        let mut drop_map = self.channel_dropped.lock();
        drop_map.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        let metrics = TransportMetrics::new("test");
        metrics.record_send(ChannelType::Control, 100);
        metrics.record_recv(ChannelType::Frames, 200);
        metrics.record_drop(ChannelType::Frames);

        let prom = metrics.export_prometheus();
        assert!(prom.contains("test_bytes_sent_total 100"));
        assert!(prom.contains("test_bytes_received_total 200"));
        assert!(prom.contains("test_frames_dropped_total 1"));
    }

    #[test]
    fn test_rtt_recording() {
        let metrics = TransportMetrics::new("test");
        metrics.record_rtt(Duration::from_millis(50));
        metrics.record_rtt(Duration::from_millis(100));

        let prom = metrics.export_prometheus();
        assert!(prom.contains("test_rtt_milliseconds"));
    }
}
