use super::*;

#[test]
fn test_repo_name() {
    // Only the OSS channel ships, and OSS doesn't autoupdate via this code path.
    assert_eq!(repo_name(Channel::Oss), "hotfuzz");
}
