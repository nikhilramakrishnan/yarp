use super::*;

#[test]
fn test_repo_name() {
    assert_eq!(repo_name(Channel::Dev), "hotfuzz-dev");
    assert_eq!(repo_name(Channel::Stable), "hotfuzz");
}
