use yarpui::{elements::MouseStateHandle, AppContext, Element};

use crate::{
    appearance::Appearance,
    cloud_object::CloudObjectMetadata,
    drive::{index::DriveIndexAction, DriveObjectType},
    server::ids::ClientId,
    themes::theme::Fill,
};

use super::{YarpDriveItem, YarpDriveItemId};

#[derive(Clone)]
pub struct YarpDriveAIFactCollection {
    id: ClientId,
}

impl YarpDriveAIFactCollection {
    pub fn new(id: ClientId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> ClientId {
        self.id
    }
}

impl YarpDriveItem for YarpDriveAIFactCollection {
    fn display_name(&self) -> Option<String> {
        Some("Rules".to_string())
    }

    fn metadata(&self) -> Option<&CloudObjectMetadata> {
        None
    }

    fn object_type(&self) -> Option<DriveObjectType> {
        Some(DriveObjectType::AIFactCollection)
    }

    fn secondary_icon(&self, _color: Option<Fill>) -> Option<Box<dyn Element>> {
        None
    }

    fn click_action(&self) -> Option<DriveIndexAction> {
        Some(DriveIndexAction::OpenAIFactCollection)
    }

    fn preview(&self, _appearance: &Appearance) -> Option<Box<dyn Element>> {
        None
    }

    fn yarp_drive_id(&self) -> YarpDriveItemId {
        YarpDriveItemId::AIFactCollection
    }

    fn sync_status_icon(
        &self,
        _sync_queue_is_dequeueing: bool,
        _hover_state: MouseStateHandle,
        _appearance: &Appearance,
    ) -> Option<Box<dyn Element>> {
        None
    }

    fn action_summary(&self, _app: &AppContext) -> Option<String> {
        None
    }

    fn clone_box(&self) -> Box<dyn YarpDriveItem> {
        Box::new(self.clone())
    }
}
