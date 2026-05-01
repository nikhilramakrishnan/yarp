//! Stub `ObjectClient` implementation for yarp.
//!
//! Yarp does not have a cloud object store. The Yarp Drive UI surfaces are
//! still wired up (workflows / notebooks / generic string objects), but every
//! mutation flows through this stub which returns a friendly error rather
//! than reaching `app.yarp.dev`.
//!
//! Reads return empty so the UI shows an empty state instead of hanging on
//! an unreachable endpoint. A future iteration may persist objects to
//! `~/.yarp/objects/` (see `LocalPaths::workflows_dir` etc.).

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_channel::Sender;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::cloud_object::{
    model::{
        actions::{ObjectActionHistory, ObjectActionType},
        generic_string_model::GenericStringObjectId,
    },
    BulkCreateCloudObjectResult, BulkCreateGenericStringObjectsRequest, CreateCloudObjectResult,
    CreateObjectRequest, GenericStringObjectFormat, GenericStringObjectUniqueKey,
    ObjectDeleteResult, ObjectMetadataUpdateResult, ObjectPermissionUpdateResult,
    ObjectPermissionsUpdateData, ObjectType, ObjectsToUpdate, Owner, Revision, ServerFolder,
    ServerMetadata, ServerNotebook, ServerObject, ServerPermissions, ServerWorkflow,
    UpdateCloudObjectResult,
};
use crate::drive::{folders::FolderId, sharing::SharingAccessLevel};
use crate::notebooks::NotebookId;
use crate::server::cloud_objects::{
    listener::ObjectUpdateMessage,
    update_manager::{GetCloudObjectResponse, InitialLoadResponse},
};
use crate::server::ids::ServerId;
use crate::server::server_api::object::{GuestIdentifier, ObjectClient};
use crate::server::sync_queue::SerializedModel;
use crate::workflows::WorkflowId;
use yarp_graphql::object_permissions::AccessLevel;

/// Stub object-client. All cloud-only operations (sharing, owner transfers,
/// trash, etc.) return `Err`. The two methods the boot path depends on -
/// `get_yarp_drive_updates` and `fetch_changed_objects` - return empty so
/// the UI never blocks on a network round-trip.
pub struct OssObjectClient;

impl OssObjectClient {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OssObjectClient {
    fn default() -> Self {
        Self::new()
    }
}

const ERR: &str = "Yarp Drive cloud sync is disabled in this build; cloud objects are not available.";

fn err<T>() -> Result<T> {
    Err(anyhow!(ERR))
}

#[async_trait]
impl ObjectClient for OssObjectClient {
    async fn create_workflow(
        &self,
        _request: CreateObjectRequest,
    ) -> Result<CreateCloudObjectResult> {
        err()
    }

    async fn update_workflow(
        &self,
        _workflow_id: WorkflowId,
        _data: SerializedModel,
        _revision: Option<Revision>,
    ) -> Result<UpdateCloudObjectResult<ServerWorkflow>> {
        err()
    }

    async fn bulk_create_generic_string_objects(
        &self,
        _owner: Owner,
        _objects: &[BulkCreateGenericStringObjectsRequest],
    ) -> Result<BulkCreateCloudObjectResult> {
        err()
    }

    async fn create_generic_string_object(
        &self,
        _format: GenericStringObjectFormat,
        _uniqueness_key: Option<GenericStringObjectUniqueKey>,
        _request: CreateObjectRequest,
    ) -> Result<CreateCloudObjectResult> {
        err()
    }

    async fn create_notebook(
        &self,
        _request: CreateObjectRequest,
    ) -> Result<CreateCloudObjectResult> {
        err()
    }

    async fn update_notebook(
        &self,
        _notebook_id: NotebookId,
        _title: Option<String>,
        _data: Option<SerializedModel>,
        _revision: Option<Revision>,
    ) -> Result<UpdateCloudObjectResult<ServerNotebook>> {
        err()
    }

    async fn create_folder(&self, _request: CreateObjectRequest) -> Result<CreateCloudObjectResult> {
        err()
    }

    async fn update_folder(
        &self,
        _folder_id: FolderId,
        _name: SerializedModel,
    ) -> Result<UpdateCloudObjectResult<ServerFolder>> {
        err()
    }

    async fn update_generic_string_object(
        &self,
        _object_id: GenericStringObjectId,
        _model: SerializedModel,
        _revision: Option<Revision>,
    ) -> Result<UpdateCloudObjectResult<Box<dyn ServerObject>>> {
        err()
    }

    async fn grab_notebook_edit_access(
        &self,
        _notebook_id: NotebookId,
    ) -> Result<ServerMetadata> {
        err()
    }

    async fn give_up_notebook_edit_access(
        &self,
        _notebook_id: NotebookId,
    ) -> Result<ServerMetadata> {
        err()
    }

    async fn get_yarp_drive_updates(
        &self,
        _message_sender: Sender<ObjectUpdateMessage>,
        stream_ready_sender: Sender<()>,
    ) -> Result<()> {
        // Signal the caller that the (empty) "stream" is ready so any
        // initialisation barriers don't block the UI.
        let _ = stream_ready_sender.send(()).await;
        Ok(())
    }

    async fn fetch_changed_objects(
        &self,
        _objects_to_update: ObjectsToUpdate,
        _force_refresh: bool,
    ) -> Result<InitialLoadResponse> {
        Ok(InitialLoadResponse::default())
    }

    async fn fetch_single_cloud_object(
        &self,
        _id: ServerId,
    ) -> Result<GetCloudObjectResponse> {
        err()
    }

    async fn transfer_notebook_owner(
        &self,
        _notebook_id: NotebookId,
        _owner: Owner,
    ) -> Result<bool> {
        err()
    }

    async fn transfer_workflow_owner(
        &self,
        _workflow_id: WorkflowId,
        _owner: Owner,
    ) -> Result<bool> {
        err()
    }

    async fn transfer_generic_string_object_owner(
        &self,
        _workflow_id: GenericStringObjectId,
        _owner: Owner,
    ) -> Result<bool> {
        err()
    }

    async fn trash_object(&self, _id: ServerId) -> Result<bool> {
        err()
    }

    async fn untrash_object(&self, _id: ServerId) -> Result<ObjectMetadataUpdateResult> {
        err()
    }

    async fn delete_object(&self, _id: ServerId) -> Result<ObjectDeleteResult> {
        err()
    }

    async fn empty_trash(&self, _owner: Owner) -> Result<ObjectDeleteResult> {
        err()
    }

    async fn move_object(
        &self,
        _id: ServerId,
        _folder_id: Option<FolderId>,
        _owner: Owner,
        _object_type: ObjectType,
    ) -> Result<bool> {
        err()
    }

    async fn record_object_action(
        &self,
        _id: ServerId,
        _action_type: ObjectActionType,
        _timestamp: DateTime<Utc>,
        _data: Option<String>,
    ) -> Result<ObjectActionHistory> {
        err()
    }

    async fn leave_object(&self, _id: ServerId) -> Result<ObjectDeleteResult> {
        err()
    }

    async fn set_object_link_permissions(
        &self,
        _object_id: ServerId,
        _access_level: SharingAccessLevel,
    ) -> Result<ObjectPermissionUpdateResult> {
        err()
    }

    async fn remove_object_link_permissions(
        &self,
        _object_id: ServerId,
    ) -> Result<ObjectPermissionUpdateResult> {
        err()
    }

    async fn add_object_guests(
        &self,
        _object_id: ServerId,
        _guest_emails: Vec<String>,
        _access_level: AccessLevel,
    ) -> Result<ObjectPermissionsUpdateData> {
        err()
    }

    async fn update_object_guests(
        &self,
        _object_id: ServerId,
        _guest_emails: Vec<String>,
        _access_level: AccessLevel,
    ) -> Result<ServerPermissions> {
        err()
    }

    async fn remove_object_guest(
        &self,
        _object_id: ServerId,
        _guest: GuestIdentifier,
    ) -> Result<ServerPermissions> {
        err()
    }

    async fn fetch_environment_last_task_run_timestamps(
        &self,
    ) -> Result<HashMap<String, DateTime<Utc>>> {
        Ok(HashMap::new())
    }
}
