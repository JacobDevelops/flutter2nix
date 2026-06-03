#[allow(unused_imports)]
use super::*;

#[tokio::test]
async fn test_tapi_invocation_timeout() {
    todo!("Phase 1: stub — config with timeout_secs:1, simulated slow JVM, expect Err containing 'timeout'")
}

#[tokio::test]
async fn test_tapi_invocation_jvm_not_found() {
    todo!("Phase 1: stub — config pointing to non-existent project, expect Err containing 'java' or 'not found'")
}
