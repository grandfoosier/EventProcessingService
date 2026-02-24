use prometheus::{Encoder, Gauge, Histogram, HistogramOpts, IntCounter, Opts, Registry, TextEncoder};

#[derive(Clone)]
pub struct Telemetry {
    pub events_ingested: IntCounter,
    pub events_deduped: IntCounter,
    pub events_processed: IntCounter,
    pub events_failed: IntCounter,
    pub queue_depth: Gauge,
    pub processing_hist: Histogram,
    pub registry: Registry,
}

impl Telemetry {
    pub fn new() -> Self {
        let registry = Registry::new();

        let events_ingested = IntCounter::with_opts(Opts::new("events_ingested_total", "Total ingested events")).unwrap();
        let events_deduped = IntCounter::with_opts(Opts::new("events_deduped_total", "Total deduped events")).unwrap();
        let events_processed = IntCounter::with_opts(Opts::new("events_processed_total", "Total processed events")).unwrap();
        let events_failed = IntCounter::with_opts(Opts::new("events_failed_total", "Total failed events")).unwrap();
        let queue_depth = Gauge::with_opts(Opts::new("queue_depth", "Queue depth")) .unwrap();
        let processing_hist = Histogram::with_opts(HistogramOpts::new("event_processing_seconds", "Event processing duration")) .unwrap();

        registry.register(Box::new(events_ingested.clone())).ok();
        registry.register(Box::new(events_deduped.clone())).ok();
        registry.register(Box::new(events_processed.clone())).ok();
        registry.register(Box::new(events_failed.clone())).ok();
        registry.register(Box::new(queue_depth.clone())).ok();
        registry.register(Box::new(processing_hist.clone())).ok();

        Telemetry { events_ingested, events_deduped, events_processed, events_failed, queue_depth, processing_hist, registry }
    }

    /// Gather metrics in Prometheus text format.
    pub fn gather(&self) -> String {
        let mut buffer = Vec::new();
        let encoder = TextEncoder::new();
        let mf = self.registry.gather();
        encoder.encode(&mf, &mut buffer).unwrap_or_default();
        String::from_utf8(buffer).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gather_contains_metric_names() {
        let t = Telemetry::new();
        t.events_ingested.inc();
        let out = t.gather();
        assert!(out.contains("events_ingested_total"), "gather output should contain metric name");
    }
}
