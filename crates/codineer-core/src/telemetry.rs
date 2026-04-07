//! Telemetry types and trait for usage analytics and diagnostics.
//!
//! Provides a pluggable sink for recording runtime metrics. The default
//! implementation is a no-op; opt-in backends (e.g., local file, remote
//! endpoint) can be configured by the user.

use std::collections::BTreeMap;
use std::time::Duration;

/// A single telemetry event.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryEvent {
    pub name: String,
    pub properties: BTreeMap<String, TelemetryValue>,
    pub timestamp: std::time::SystemTime,
}

/// Supported telemetry property value types.
#[derive(Debug, Clone, PartialEq)]
pub enum TelemetryValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Duration(Duration),
}

impl TelemetryEvent {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            properties: BTreeMap::new(),
            timestamp: std::time::SystemTime::now(),
        }
    }

    #[must_use]
    pub fn with_property(mut self, key: impl Into<String>, value: TelemetryValue) -> Self {
        self.properties.insert(key.into(), value);
        self
    }
}

/// Trait for telemetry backends.
///
/// Implementations decide how/where to store or send events.
/// All methods are synchronous to avoid async in the core crate.
pub trait TelemetrySink: Send {
    fn record(&mut self, event: TelemetryEvent);
    fn flush(&mut self);
    fn is_enabled(&self) -> bool;
}

/// No-op telemetry sink (default when telemetry is not configured).
#[derive(Default)]
pub struct NullSink;

impl TelemetrySink for NullSink {
    fn record(&mut self, _event: TelemetryEvent) {}
    fn flush(&mut self) {}
    fn is_enabled(&self) -> bool {
        false
    }
}

/// Collects events in memory (useful for testing).
#[derive(Debug, Default)]
pub struct MemorySink {
    pub events: Vec<TelemetryEvent>,
}

impl TelemetrySink for MemorySink {
    fn record(&mut self, event: TelemetryEvent) {
        self.events.push(event);
    }
    fn flush(&mut self) {}
    fn is_enabled(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_sink_is_disabled() {
        let sink = NullSink;
        assert!(!sink.is_enabled());
    }

    #[test]
    fn memory_sink_collects_events() {
        let mut sink = MemorySink::default();
        assert!(sink.is_enabled());

        let event = TelemetryEvent::new("test_event")
            .with_property("model", TelemetryValue::String("opus".to_string()))
            .with_property("tokens", TelemetryValue::Int(1024));
        sink.record(event);

        assert_eq!(sink.events.len(), 1);
        assert_eq!(sink.events[0].name, "test_event");
        assert_eq!(
            sink.events[0].properties.get("model"),
            Some(&TelemetryValue::String("opus".to_string()))
        );
    }

    #[test]
    fn event_builder_chain() {
        let event = TelemetryEvent::new("api_call")
            .with_property(
                "latency",
                TelemetryValue::Duration(Duration::from_millis(150)),
            )
            .with_property("success", TelemetryValue::Bool(true));
        assert_eq!(event.properties.len(), 2);
    }
}
