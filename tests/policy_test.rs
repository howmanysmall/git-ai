#[cfg(test)]
mod tests {
    use git_ai::policy::outbound_network_reporting_disabled;

    #[test]
    fn test_outbound_network_reporting_disabled_is_true() {
        assert!(outbound_network_reporting_disabled(), "Policy should be disabled by default");
    }
}
