use crate::repos::test_repo::TestRepo;

mod repos;

#[test]
fn test_telemetry_disabled_by_policy() {
    let repo = TestRepo::new();

    // Test flush-logs
    let output = repo.git_ai(&["flush-logs"]).unwrap();
    assert!(output.contains("Telemetry is disabled by fork policy"), "Output was: {}", output);
}

#[test]
fn test_cas_upload_disabled_by_policy() {
    let repo = TestRepo::new();

    // Test flush-cas
    let output = repo.git_ai(&["flush-cas"]).unwrap();
    assert!(output.contains("CAS upload is disabled by fork policy"), "Output was: {}", output);
}
