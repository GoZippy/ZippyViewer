use prometheus::{
    register_counter_with_registry, register_gauge_with_registry, register_histogram_with_registry,
    Counter, Gauge, Histogram, Registry,
};
use std::sync::Arc;

pub struct MailboxMetrics {
    pub active_mailboxes: Gauge,
    pub total_messages: Gauge,
    pub messages_posted: Counter,
    pub messages_delivered: Counter,
    pub messages_evicted: Counter,
    pub request_latency: Histogram,
    pub rate_limit_hits: Counter,
    pub error_counts: Counter,
    pub registry: Arc<Registry>,
}

impl MailboxMetrics {
    pub fn new() -> anyhow::Result<Self> {
        let registry = Arc::new(Registry::new());

        let active_mailboxes = register_gauge_with_registry!(
            "zrc_rendezvous_active_mailboxes",
            "Number of active mailboxes",
            registry
        )?;

        let total_messages = register_gauge_with_registry!(
            "zrc_rendezvous_total_messages",
            "Total number of messages in all mailboxes",
            registry
        )?;

        let messages_posted = register_counter_with_registry!(
            "zrc_rendezvous_messages_posted_total",
            "Total number of messages posted",
            registry
        )?;

        let messages_delivered = register_counter_with_registry!(
            "zrc_rendezvous_messages_delivered_total",
            "Total number of messages delivered",
            registry
        )?;

        let messages_evicted = register_counter_with_registry!(
            "zrc_rendezvous_messages_evicted_total",
            "Total number of messages evicted",
            registry
        )?;

        let request_latency = register_histogram_with_registry!(
            "zrc_rendezvous_request_latency_seconds",
            "Request latency in seconds",
            registry
        )?;

        let rate_limit_hits = register_counter_with_registry!(
            "zrc_rendezvous_rate_limit_hits_total",
            "Total number of rate limit hits",
            registry
        )?;

        let error_counts = register_counter_with_registry!(
            "zrc_rendezvous_errors_total",
            "Total number of errors",
            registry
        )?;

        Ok(Self {
            active_mailboxes,
            total_messages,
            messages_posted,
            messages_delivered,
            messages_evicted,
            request_latency,
            rate_limit_hits,
            error_counts,
            registry,
        })
    }

    pub fn export_prometheus(&self) -> String {
        use prometheus::Encoder;
        let encoder = prometheus::TextEncoder::new();
        let mut buffer = Vec::new();
        encoder.encode(&self.registry.gather(), &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}

impl Default for MailboxMetrics {
    fn default() -> Self {
        Self::new().unwrap()
    }
}
