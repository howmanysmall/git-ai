//! Fork-level policy for disabling outbound network reporting
//!
//! This module provides a central policy gate that controls whether
//! telemetry and prompt uploads are allowed. In this fork, all outbound
//! network reporting is disabled by default.

/// Returns true if outbound network reporting (telemetry and prompt uploads) is disabled.
///
/// In this fork, this function always returns `true` to disable:
/// - Sentry telemetry
/// - PostHog analytics
/// - CAS (Content-Addressable Storage) prompt uploads
///
/// This ensures no data is sent to external services regardless of
/// configuration, API keys, or login state.
pub fn outbound_network_reporting_disabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outbound_network_reporting_is_disabled() {
        // This fork disables all outbound network reporting by policy
        assert!(
            outbound_network_reporting_disabled(),
            "Fork policy requires outbound network reporting to be disabled"
        );
    }
}
