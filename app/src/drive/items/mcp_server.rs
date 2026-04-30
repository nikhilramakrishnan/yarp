use super::{YarpDriveItem, YarpDriveItemId};
use crate::{
    ai::mcp::CloudMCPServer,
    appearance::Appearance,
    cloud_object::CloudObjectMetadata,
    drive::{index::DriveIndexAction, CloudObjectTypeAndId, DriveObjectType},
    themes::theme::Fill,
};
use yarpui::{elements::MouseStateHandle, AppContext, Element};

#[derive(Clone)]
pub struct YarpDriveMCPServer {
    id: CloudObjectTypeAndId,
    mcp_server: CloudMCPServer,
}

impl YarpDriveMCPServer {
    pub fn new(id: CloudObjectTypeAndId, mcp_server: CloudMCPServer) -> Self {
        Self { id, mcp_server }
    }
}

impl YarpDriveItem for YarpDriveMCPServer {
    fn display_name(&self) -> Option<String> {
        Some(self.mcp_server.model().string_model.name.clone())
    }
    fn metadata(&self) -> Option<&CloudObjectMetadata> {
        Some(&self.mcp_server.metadata)
    }

    fn object_type(&self) -> Option<DriveObjectType> {
        Some(DriveObjectType::MCPServer)
    }

    fn secondary_icon(&self, _color: Option<Fill>) -> Option<Box<dyn Element>> {
        None
    }

    fn click_action(&self) -> Option<DriveIndexAction> {
        Some(DriveIndexAction::OpenMCPServerCollection)
    }

    fn preview(&self, _appearance: &Appearance) -> Option<Box<dyn Element>> {
        // TODO
        None
    }

    fn yarp_drive_id(&self) -> YarpDriveItemId {
        YarpDriveItemId::Object(self.id)
    }

    fn sync_status_icon(
        &self,
        sync_queue_is_dequeueing: bool,
        hover_state: MouseStateHandle,
        appearance: &Appearance,
    ) -> Option<Box<dyn Element>> {
        self.mcp_server
            .metadata
            .pending_changes_statuses
            .render_icon(sync_queue_is_dequeueing, hover_state, appearance)
    }

    fn action_summary(&self, _app: &AppContext) -> Option<String> {
        None
    }

    fn clone_box(&self) -> Box<dyn YarpDriveItem> {
        Box::new(self.clone())
    }
}
