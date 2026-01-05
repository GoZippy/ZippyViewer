//! Metrics collection and export

use prometheus::{
    Counter, Gauge, Histogram, HistogramOpts, Opts, Registry,
    Encoder, TextEncoder,
};
use std::time::Instant;
use std::sync::{Mutex, atomic::{AtomicU64, Ordering}};

/// Allocation metrics
pub struct AllocationMetrics {
    active_allocations: Gauge,
    total_allocations: Counter,
    bytes_forwarded: Counter,
    packets_forwarded: Counter,
    allocation_duration: Histogram,
    bandwidth_usage: Gauge,
    quota_usage: Gauge,
    quota_exceeded: Counter,
    rate_limit_drops: Counter,
    connection_count: Gauge,
    error_count: Counter,
    rate_limit_hits: Counter,
    geographic_distribution: std::collections::HashMap<String, Gauge>,
    registry: Registry,
    
    // Rate calculation state
    start_time: Instant,
    peak_bandwidth: AtomicU64,
    last_update_time: Mutex<Instant>,
    last_bytes_count: AtomicU64,
}

impl AllocationMetrics {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        let active_allocations = Gauge::with_opts(Opts::new(
            "zrc_relay_active_allocations",
            "Number of active allocations",
        ))?;
        registry.register(Box::new(active_allocations.clone()))?;

        let total_allocations = Counter::with_opts(Opts::new(
            "zrc_relay_total_allocations",
            "Total number of allocations created",
        ))?;
        registry.register(Box::new(total_allocations.clone()))?;

        let bytes_forwarded = Counter::with_opts(Opts::new(
            "zrc_relay_bytes_forwarded",
            "Total bytes forwarded",
        ))?;
        registry.register(Box::new(bytes_forwarded.clone()))?;

        let packets_forwarded = Counter::with_opts(Opts::new(
            "zrc_relay_packets_forwarded",
            "Total packets forwarded",
        ))?;
        registry.register(Box::new(packets_forwarded.clone()))?;

        let allocation_duration = Histogram::with_opts(HistogramOpts::new(
            "zrc_relay_allocation_duration_seconds",
            "Allocation duration in seconds",
        ))?;
        registry.register(Box::new(allocation_duration.clone()))?;

        let bandwidth_usage = Gauge::with_opts(Opts::new(
            "zrc_relay_bandwidth_usage_bytes_per_sec",
            "Current bandwidth usage in bytes per second",
        ))?;
        registry.register(Box::new(bandwidth_usage.clone()))?;

        let quota_usage = Gauge::with_opts(Opts::new(
            "zrc_relay_quota_usage_bytes",
            "Current quota usage in bytes",
        ))?;
        registry.register(Box::new(quota_usage.clone()))?;

        let quota_exceeded = Counter::with_opts(Opts::new(
            "zrc_relay_quota_exceeded_total",
            "Total number of allocations terminated due to quota exceeded",
        ))?;
        registry.register(Box::new(quota_exceeded.clone()))?;

        let rate_limit_drops = Counter::with_opts(Opts::new(
            "zrc_relay_rate_limit_drops_total",
            "Total packets dropped due to rate limiting",
        ))?;
        registry.register(Box::new(rate_limit_drops.clone()))?;

        let connection_count = Gauge::with_opts(Opts::new(
            "zrc_relay_connection_count",
            "Current number of connections",
        ))?;
        registry.register(Box::new(connection_count.clone()))?;

        let error_count = Counter::with_opts(Opts::new(
            "zrc_relay_errors_total",
            "Total number of errors",
        ))?;
        registry.register(Box::new(error_count.clone()))?;

        let rate_limit_hits = Counter::with_opts(Opts::new(
            "zrc_relay_rate_limit_hits_total",
            "Total number of rate limit hits",
        ))?;
        registry.register(Box::new(rate_limit_hits.clone()))?;

        Ok(Self {
            active_allocations,
            total_allocations,
            bytes_forwarded,
            packets_forwarded,
            allocation_duration,
            bandwidth_usage,
            quota_usage,
            quota_exceeded,
            rate_limit_drops,
            connection_count,
            error_count,
            rate_limit_hits,
            geographic_distribution: std::collections::HashMap::new(),
            registry,
            
            start_time: Instant::now(),
            peak_bandwidth: AtomicU64::new(0),
            last_update_time: Mutex::new(Instant::now()),
            last_bytes_count: AtomicU64::new(0),
        })
    }

    /// Record geographic distribution (for multi-relay deployments)
    pub fn record_geographic_allocation(&mut self, region: &str) {
        let gauge = self.geographic_distribution
            .entry(region.to_string())
            .or_insert_with(|| {
                let g = Gauge::with_opts(Opts::new(
                    format!("zrc_relay_allocations_by_region_{}", region.replace("-", "_")),
                    format!("Active allocations in region {}", region),
                )).unwrap();
                self.registry.register(Box::new(g.clone())).unwrap();
                g
            });
        gauge.inc();
    }

    /// Remove geographic allocation
    pub fn remove_geographic_allocation(&mut self, region: &str) {
        if let Some(gauge) = self.geographic_distribution.get(region) {
            gauge.dec();
        }
    }

    pub fn record_allocation_created(&self) {
        self.total_allocations.inc();
        self.active_allocations.inc();
    }

    pub fn record_allocation_terminated(&self, duration: std::time::Duration) {
        self.active_allocations.dec();
        self.allocation_duration.observe(duration.as_secs_f64());
    }

    pub fn record_forward(&self, bytes: usize) {
        self.bytes_forwarded.inc_by(bytes as f64);
        self.packets_forwarded.inc();
    }

    pub fn record_quota_exceeded(&self) {
        self.quota_exceeded.inc();
    }

    pub fn record_rate_limit_drop(&self) {
        self.rate_limit_drops.inc();
        self.rate_limit_hits.inc();
    }

    pub fn record_error(&self) {
        self.error_count.inc();
    }

    pub fn set_active_allocations(&self, count: usize) {
        self.active_allocations.set(count as f64);
    }

    pub fn set_connection_count(&self, count: usize) {
        self.connection_count.set(count as f64);
    }

    pub fn set_bandwidth_usage(&self, bytes_per_sec: u64) {
        self.bandwidth_usage.set(bytes_per_sec as f64);
    }

    pub fn set_quota_usage(&self, bytes: u64) {
        self.quota_usage.set(bytes as f64);
    }

    /// Export Prometheus format
    pub fn export(&self) -> Result<String, prometheus::Error> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8_lossy(&buffer).to_string())
    }

    /// Get total allocations count
    pub fn total_allocations(&self) -> f64 {
        self.total_allocations.get()
    }

    /// Get bytes forwarded count
    pub fn bytes_forwarded(&self) -> f64 {
        self.bytes_forwarded.get()
    }

    /// Get packets forwarded count
    pub fn packets_forwarded(&self) -> f64 {
        self.packets_forwarded.get()
    }

    /// Update bandwidth rate calculation
    /// Should be called periodically (e.g. every few seconds)
    pub fn update_rate_calc(&self) {
        let now = Instant::now();
        let mut last_time = self.last_update_time.lock().unwrap();
        let elapsed = now.duration_since(*last_time);
        
        // Only update if at least 1 second has passed to avoid jitter
        if elapsed.as_secs_f64() < 1.0 {
            return;
        }

        let current_bytes = self.bytes_forwarded.get() as u64;
        let last_bytes = self.last_bytes_count.swap(current_bytes, Ordering::Relaxed);
        
        let diff = current_bytes.saturating_sub(last_bytes);
        let rate = (diff as f64 / elapsed.as_secs_f64()) as u64;
        
        self.set_bandwidth_usage(rate);
        
        let current_peak = self.peak_bandwidth.load(Ordering::Relaxed);
        if rate > current_peak {
            self.peak_bandwidth.store(rate, Ordering::Relaxed);
        }
        
        *last_time = now;
    }

    pub fn uptime(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
    
    pub fn peak_bandwidth(&self) -> u64 {
        self.peak_bandwidth.load(Ordering::Relaxed)
    }
    
    pub fn current_bandwidth(&self) -> u64 {
        self.bandwidth_usage.get() as u64
    }
    
    pub fn average_bandwidth(&self) -> u64 {
        let uptime = self.uptime().as_secs_f64();
        if uptime > 0.0 {
            (self.bytes_forwarded.get() / uptime) as u64
        } else {
            0
        }
    }
}
