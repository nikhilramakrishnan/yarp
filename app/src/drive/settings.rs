use settings::{
    macros::define_settings_group, RespectUserSyncSetting, SupportedPlatforms, SyncToCloud,
};
use yarp_core::features::FeatureFlag;

use super::DriveSortOrder;

pub const HAS_AUTO_OPENED_WELCOME_FOLDER: &str = "HasAutoOpenedWelcomeFolder";

define_settings_group!(YarpDriveSettings, settings: [
    sorting_choice: YarpDriveSortingChoice {
        type: DriveSortOrder,
        default: DriveSortOrder::ByObjectType,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "warp_drive.sorting_choice",
        description: "The sort order for items in Yarp Drive.",
    },
    sharing_onboarding_block_shown: YarpDriveSharingOnboardingBlockShown {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    },
    // Controls whether Yarp Drive appears in the tools panel, command palette, and command search.
    enable_yarp_drive: EnableYarpDrive {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "warp_drive.enabled",
        description: "Whether Yarp Drive is enabled.",
    },
]);

impl YarpDriveSettings {
    /// Returns whether Yarp Drive should be considered enabled.
    /// Returns `false` when the user is anonymous or fully logged out,
    /// regardless of the user setting.
    pub fn is_yarp_drive_enabled(app: &yarpui::AppContext) -> bool {
        use yarpui::SingletonEntity as _;
        let is_anonymous_or_logged_out = FeatureFlag::SkipFirebaseAnonymousUser.is_enabled()
            && crate::auth::AuthStateProvider::as_ref(app)
                .get()
                .is_anonymous_or_logged_out();
        *Self::as_ref(app).enable_yarp_drive && !is_anonymous_or_logged_out
    }
}
