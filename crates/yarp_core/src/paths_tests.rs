use dirs::home_dir;

use super::*;

#[test]
fn test_data_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    // ChannelState, by default, is configured for Channel::Oss.
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(data_dir(), home_dir.join(".yarp"));
        } else if #[cfg(target_os = "linux")] {
            assert_eq!(data_dir(), home_dir.join(".local/share/yarp"));
        } else if #[cfg(windows)] {
            assert_eq!(data_dir(), home_dir.join("AppData\\Roaming\\yarp\\YarpOss\\data"));
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_config_local_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    // ChannelState, by default, is configured for Channel::Oss.
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(config_local_dir(), home_dir.join(".yarp"));
        } else if #[cfg(target_os = "linux")] {
            assert_eq!(config_local_dir(), home_dir.join(".config/yarp"));
        } else if #[cfg(windows)] {
            assert_eq!(config_local_dir(), home_dir.join("AppData\\Local\\yarp\\YarpOss\\config"));
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_warp_home_config_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    let expected_dir_name = match ChannelState::data_profile() {
        Some(data_profile) => format!(".yarp-{data_profile}"),
        None => ".yarp".to_string(),
    };

    assert_eq!(
        yarp_home_config_dir(),
        Some(home_dir.join(expected_dir_name))
    );
}

#[test]
fn test_warp_home_skills_and_mcp_paths() {
    let Some(config_dir) = yarp_home_config_dir() else {
        panic!("Should be able to compute Yarp home config directory");
    };

    assert_eq!(yarp_home_skills_dir(), Some(config_dir.join("skills")));
    assert_eq!(
        yarp_home_mcp_config_file_path(),
        Some(config_dir.join(".mcp.json"))
    );
}
#[test]
fn test_cache_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    // ChannelState, by default, is configured for Channel::Oss.
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(cache_dir(), home_dir.join("Library/Application Support/dev.yarp.YarpOss"));
        } else if #[cfg(target_os = "linux")] {
            assert_eq!(cache_dir(), home_dir.join(".cache/yarp"));
        } else if #[cfg(windows)] {
            assert_eq!(cache_dir(), home_dir.join("AppData\\Local\\yarp\\YarpOss\\cache"));
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_state_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    cfg_if::cfg_if! {
        // ChannelState, by default, is configured for Channel::Oss.
        if #[cfg(target_os = "macos")] {
            assert_eq!(state_dir(), home_dir.join("Library/Application Support/dev.yarp.YarpOss"));
        } else if #[cfg(target_os = "linux")] {
            assert_eq!(state_dir(), home_dir.join(".local/state/yarp"));
        } else if #[cfg(windows)] {
            assert_eq!(state_dir(), home_dir.join("AppData\\Local\\yarp\\YarpOss\\data"));
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_project_path_for_warp_app_id() {
    let project_dirs = project_dirs_for_app_id(AppId::new("dev", "yarp", "Yarp"), None)
        .expect("should be able to compute project dirs");
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(project_dirs.project_path(), "dev.yarp.Yarp");
        } else if #[cfg(target_os = "linux")] {
            assert_eq!(project_dirs.project_path(), "yarp-terminal");
        } else if #[cfg(windows)] {
            assert_eq!(project_dirs.project_path(), "yarp\\Yarp");
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_project_path_for_warp_dev_app_id() {
    let project_dirs = project_dirs_for_app_id(AppId::new("dev", "yarp", "YarpDev"), None)
        .expect("should be able to compute project dirs");
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(project_dirs.project_path(), "dev.yarp.YarpDev");
        } else if #[cfg(target_os = "linux")] {
            assert_eq!(project_dirs.project_path(), "yarp-terminal-dev");
        } else if #[cfg(windows)] {
            assert_eq!(project_dirs.project_path(), "yarp\\YarpDev");
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_project_path_for_oss_app_id() {
    let project_dirs = project_dirs_for_app_id(AppId::new("dev", "yarp", "YarpOss"), None)
        .expect("should be able to compute project dirs");
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(project_dirs.project_path(), "dev.yarp.YarpOss");
        } else if #[cfg(target_os = "linux")] {
            assert_eq!(project_dirs.project_path(), "yarp");
        } else if #[cfg(windows)] {
            assert_eq!(project_dirs.project_path(), "yarp\\YarpOss");
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}
