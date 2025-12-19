//! Trace Export

use serde::{Deserialize, Serialize};

use super::collector::TraceReport;

/// Export format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceFormat {
    /// JSON format
    Json,
    /// Pretty-printed JSON
    JsonPretty,
    /// Compact summary
    Summary,
}

/// Trace exporter
pub struct TraceExporter;

impl TraceExporter {
    /// Export to JSON
    pub fn to_json(report: &TraceReport) -> Result<String, serde_json::Error> {
        serde_json::to_string(report)
    }

    /// Export to pretty JSON
    pub fn to_json_pretty(report: &TraceReport) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(report)
    }

    /// Export to summary format
    pub fn to_summary(report: &TraceReport) -> String {
        let mut lines = Vec::new();

        lines.push(format!("Trace Report: {}", report.workflow_name));
        lines.push(format!("ID: {}", report.id));
        lines.push(format!(
            "Status: {}",
            if report.success { "SUCCESS" } else { "FAILED" }
        ));

        if let Some(ref error) = report.error {
            lines.push(format!("Error: {}", error));
        }

        lines.push(String::new());
        lines.push("Summary:".to_string());
        lines.push(format!("  Steps: {}", report.summary.total_steps));
        lines.push(format!(
            "  Successful: {}",
            report.summary.successful_steps
        ));
        lines.push(format!("  Failed: {}", report.summary.failed_steps));
        lines.push(format!(
            "  Duration: {}ms",
            report.summary.total_duration_ms
        ));

        lines.push(String::new());
        lines.push("Token Usage:".to_string());
        lines.push(format!("  Prompt: {}", report.summary.prompt_tokens));
        lines.push(format!(
            "  Completion: {}",
            report.summary.completion_tokens
        ));
        lines.push(format!("  Total: {}", report.summary.total_tokens));

        if report.summary.estimated_cost_usd > 0.0 {
            lines.push(format!(
                "  Est. Cost: ${:.6}",
                report.summary.estimated_cost_usd
            ));
        }

        if !report.tags.is_empty() {
            lines.push(String::new());
            lines.push("Tags:".to_string());
            for (key, value) in &report.tags {
                lines.push(format!("  {}: {}", key, value));
            }
        }

        lines.join("\n")
    }

    /// Export in specified format
    pub fn export(report: &TraceReport, format: TraceFormat) -> Result<String, serde_json::Error> {
        match format {
            TraceFormat::Json => Self::to_json(report),
            TraceFormat::JsonPretty => Self::to_json_pretty(report),
            TraceFormat::Summary => Ok(Self::to_summary(report)),
        }
    }
}

/// OpenTelemetry-compatible span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OTelSpan {
    /// Trace ID
    pub trace_id: String,
    /// Span ID
    pub span_id: String,
    /// Parent span ID
    pub parent_span_id: Option<String>,
    /// Operation name
    pub operation_name: String,
    /// Start time (Unix timestamp in microseconds)
    pub start_time_us: u64,
    /// Duration in microseconds
    pub duration_us: u64,
    /// Status
    pub status: SpanStatus,
    /// Attributes
    pub attributes: std::collections::HashMap<String, serde_json::Value>,
}

/// Span status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SpanStatus {
    Ok,
    Error,
    Unset,
}

impl TraceExporter {
    /// Convert report to OpenTelemetry spans
    pub fn to_otel_spans(report: &TraceReport) -> Vec<OTelSpan> {
        let mut spans = Vec::new();
        let trace_id = &report.id;

        // Root span
        let root_span_id = format!("{}-root", trace_id);
        let start_time = report
            .started_at
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        let end_time = report
            .completed_at
            .unwrap_or(report.started_at)
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        let mut root_attrs = std::collections::HashMap::new();
        root_attrs.insert(
            "workflow.name".to_string(),
            serde_json::json!(report.workflow_name),
        );
        root_attrs.insert(
            "workflow.steps".to_string(),
            serde_json::json!(report.summary.total_steps),
        );
        root_attrs.insert(
            "workflow.tokens".to_string(),
            serde_json::json!(report.summary.total_tokens),
        );

        for (key, value) in &report.tags {
            root_attrs.insert(format!("tag.{}", key), serde_json::json!(value));
        }

        spans.push(OTelSpan {
            trace_id: trace_id.clone(),
            span_id: root_span_id.clone(),
            parent_span_id: None,
            operation_name: report.workflow_name.clone(),
            start_time_us: start_time,
            duration_us: end_time.saturating_sub(start_time),
            status: if report.success {
                SpanStatus::Ok
            } else {
                SpanStatus::Error
            },
            attributes: root_attrs,
        });

        // Step spans
        let mut offset = 0u64;
        for (i, step) in report.steps.iter().enumerate() {
            let span_id = format!("{}-step-{}", trace_id, i);

            let mut attrs = std::collections::HashMap::new();
            attrs.insert(
                "step.name".to_string(),
                serde_json::json!(step.step_name),
            );

            if let Some(ref usage) = step.token_usage {
                attrs.insert(
                    "step.prompt_tokens".to_string(),
                    serde_json::json!(usage.prompt_tokens),
                );
                attrs.insert(
                    "step.completion_tokens".to_string(),
                    serde_json::json!(usage.completion_tokens),
                );
            }

            if let Some(ref error) = step.error {
                attrs.insert("error.message".to_string(), serde_json::json!(error));
            }

            spans.push(OTelSpan {
                trace_id: trace_id.clone(),
                span_id,
                parent_span_id: Some(root_span_id.clone()),
                operation_name: step.step_name.clone(),
                start_time_us: start_time + offset * 1000,
                duration_us: step.duration_ms * 1000,
                status: if step.success {
                    SpanStatus::Ok
                } else {
                    SpanStatus::Error
                },
                attributes: attrs,
            });

            offset += step.duration_ms;
        }

        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracing::collector::TraceCollector;
    use crate::workflow::StepTrace;

    fn create_test_report() -> TraceReport {
        let mut collector = TraceCollector::new("test-workflow");
        collector.add_tag("env", "test");
        collector.add_step(
            StepTrace::success("step1", serde_json::json!({}), serde_json::json!({}), 100)
                .with_token_usage(100, 50, 150),
        );
        collector.finalize()
    }

    #[test]
    fn test_export_json() {
        let report = create_test_report();
        let json = TraceExporter::to_json(&report).unwrap();

        assert!(json.contains("test-workflow"));
        assert!(json.contains("step1"));
    }

    #[test]
    fn test_export_summary() {
        let report = create_test_report();
        let summary = TraceExporter::to_summary(&report);

        assert!(summary.contains("Trace Report: test-workflow"));
        assert!(summary.contains("Status: SUCCESS"));
        assert!(summary.contains("Steps: 1"));
        assert!(summary.contains("Prompt: 100"));
    }

    #[test]
    fn test_export_format() {
        let report = create_test_report();

        let json = TraceExporter::export(&report, TraceFormat::Json).unwrap();
        assert!(json.starts_with('{'));

        let summary = TraceExporter::export(&report, TraceFormat::Summary).unwrap();
        assert!(summary.contains("Trace Report"));
    }

    #[test]
    fn test_otel_spans() {
        let report = create_test_report();
        let spans = TraceExporter::to_otel_spans(&report);

        // Should have root span + 1 step span
        assert_eq!(spans.len(), 2);

        // Root span
        assert!(spans[0].parent_span_id.is_none());
        assert_eq!(spans[0].status, SpanStatus::Ok);

        // Step span
        assert!(spans[1].parent_span_id.is_some());
        assert_eq!(spans[1].operation_name, "step1");
    }
}
