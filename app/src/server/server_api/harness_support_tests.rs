use std::{collections::HashMap, fs};

use futures::executor::block_on;
use tempfile::tempdir;

use super::{upload_to_target, UploadTarget};
use crate::ai::artifacts::Artifact;

/// Assert that `Artifact`s serialize to the expected format for the /harness-support/report-artifact
/// endpoint.
/// If `Artifact` serialization changes, this test will catch it.
#[test]
fn pull_request_artifact_serializes_to_expected_wire_format() {
    let artifact = Artifact::PullRequest {
        url: "https://github.com/org/repo/pull/42".to_string(),
        branch: "feature-branch".to_string(),
        repo: Some("repo".to_string()),
        number: Some(42),
    };
    let json = serde_json::to_value(&artifact).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "artifact_type": "PULL_REQUEST",
            "data": {
                "url": "https://github.com/org/repo/pull/42",
                "branch": "feature-branch"
            }
        })
    );
}

#[test]
fn local_file_upload_target_writes_body_to_disk() {
    block_on(async {
        let tempdir = tempdir().unwrap();
        let path = tempdir.path().join("nested").join("transcript.json");
        let target = UploadTarget {
            url: url::Url::from_file_path(&path).unwrap().to_string(),
            method: "PUT".to_string(),
            headers: HashMap::new(),
        };
        let client = http_client::Client::new_for_test();

        upload_to_target(&client, &target, br#"{"ok":true}"#.to_vec())
            .await
            .unwrap();

        assert_eq!(fs::read(path).unwrap(), br#"{"ok":true}"#);
    });
}
