// yarp: this fork has no warp.dev backend, so the cloud-object sync queue is
// inert. The original implementation (~1,900 LOC) drove serial CRUD against
// the server with dependency tracking, retry/backoff, and revision
// reconciliation. None of that runs here. We keep the public surface
// (`QueueItem`, `SerializedModel`, `SyncQueueEvent`, `SyncQueue`) because
// callers across the app construct queue items and observe the singleton,
// but every operation is a no-op and `SyncQueueEvent` has no variants — no
// event is ever emitted.

use chrono::{DateTime, Utc};
use derivative::Derivative;
use std::sync::Arc;
use uuid::Uuid;
use yarpui::{Entity, ModelContext, SingletonEntity};

use super::ids::{ClientId, SyncId};

use crate::ai::mcp::templatable::CloudTemplatableMCPServerModel;
use crate::server::cloud_objects::update_manager::InitiatedBy;
use crate::{
    ai::cloud_agent_config::CloudAgentConfigModel,
    ai::cloud_environments::CloudAmbientAgentEnvironmentModel,
    ai::{
        ambient_agents::scheduled::CloudScheduledAmbientAgentModel,
        execution_profiles::CloudAIExecutionProfileModel, facts::CloudAIFactModel,
        mcp::CloudMCPServerModel,
    },
    cloud_object::{
        model::actions::{ObjectAction, ObjectActionSubtype, ObjectActionType},
        CloudObject, CloudObjectEventEntrypoint, GenericStringObjectFormat,
        GenericStringObjectUniqueKey, ObjectType, Owner, Revision,
    },
    drive::{folders::CloudFolderModel, CloudObjectTypeAndId},
    env_vars::CloudEnvVarCollectionModel,
    notebooks::CloudNotebookModel,
    settings::cloud_preferences::CloudPreferenceModel,
    workflows::{workflow_enum::CloudWorkflowEnumModel, CloudWorkflowModel},
};

// A newtype for a serialized model that wraps a plain string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializedModel(String);

impl SerializedModel {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn model_as_str(&self) -> &str {
        &self.0
    }

    pub fn take(self) -> String {
        self.0
    }
}

impl From<String> for SerializedModel {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct GenericStringObjectToCreate {
    pub id: ClientId,
    pub format: GenericStringObjectFormat,
    pub serialized_model: Arc<SerializedModel>,
    pub initial_folder_id: Option<SyncId>,
    pub entrypoint: CloudObjectEventEntrypoint,
    pub uniqueness_key: Option<GenericStringObjectUniqueKey>,
    pub initiated_by: InitiatedBy,
}

/// An ID for a `QueueItem` in the sync queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueueItemId(Uuid);

impl QueueItemId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for QueueItemId {
    fn default() -> Self {
        Self::new()
    }
}

/// Item that the sync queue accepts. yarp drops every variant on the floor;
/// the type is retained because callers construct these as part of the
/// cloud-object update plumbing.
#[derive(Derivative, Debug)]
#[derivative(PartialEq, Eq, Clone)]
pub enum QueueItem {
    CreateObject {
        object_type: ObjectType,
        owner: Owner,
        id: ClientId,
        title: Option<Arc<String>>,
        serialized_model: Option<Arc<SerializedModel>>,
        initial_folder_id: Option<SyncId>,
        entrypoint: CloudObjectEventEntrypoint,
        initiated_by: InitiatedBy,
    },
    CreateWorkflow {
        object_type: ObjectType,
        owner: Owner,
        id: ClientId,
        #[derivative(PartialEq = "ignore")]
        model: Arc<CloudWorkflowModel>,
        initial_folder_id: Option<SyncId>,
        entrypoint: CloudObjectEventEntrypoint,
        initiated_by: InitiatedBy,
    },
    BulkCreateGenericStringObjects {
        owner: Owner,
        objects: Vec<GenericStringObjectToCreate>,
    },
    UpdateNotebook {
        model: Arc<CloudNotebookModel>,
        id: SyncId,
        revision: Option<Revision>,
    },
    UpdateWorkflow {
        model: Arc<CloudWorkflowModel>,
        id: SyncId,
        revision: Option<Revision>,
    },
    UpdateFolder {
        id: SyncId,
        model: Arc<CloudFolderModel>,
    },
    UpdateCloudPreferences {
        model: Arc<CloudPreferenceModel>,
        id: SyncId,
        revision: Option<Revision>,
    },
    UpdateEnvVarCollection {
        model: Arc<CloudEnvVarCollectionModel>,
        id: SyncId,
        revision: Option<Revision>,
    },
    UpdateWorkflowEnum {
        model: Arc<CloudWorkflowEnumModel>,
        id: SyncId,
        revision: Option<Revision>,
    },
    UpdateAIFact {
        model: Arc<CloudAIFactModel>,
        id: SyncId,
        revision: Option<Revision>,
    },
    UpdateMCPServer {
        model: Arc<CloudMCPServerModel>,
        id: SyncId,
        revision: Option<Revision>,
    },
    UpdateAIExecutionProfile {
        model: Arc<CloudAIExecutionProfileModel>,
        id: SyncId,
        revision: Option<Revision>,
    },
    UpdateTemplatableMCPServer {
        model: Arc<CloudTemplatableMCPServerModel>,
        id: SyncId,
        revision: Option<Revision>,
    },
    UpdateCloudEnvironment {
        model: Arc<CloudAmbientAgentEnvironmentModel>,
        id: SyncId,
        revision: Option<Revision>,
    },
    UpdateScheduledAmbientAgent {
        model: Arc<CloudScheduledAmbientAgentModel>,
        id: SyncId,
        revision: Option<Revision>,
    },
    UpdateCloudAgentConfig {
        model: Arc<CloudAgentConfigModel>,
        id: SyncId,
        revision: Option<Revision>,
    },
    RecordObjectAction {
        id_and_type: CloudObjectTypeAndId,
        action_type: ObjectActionType,
        action_timestamp: DateTime<Utc>,
        data: Option<String>,
    },
}

impl QueueItem {
    pub fn from_cached_objects(
        objects: impl Iterator<Item = Box<dyn CloudObject>>,
    ) -> Vec<QueueItem> {
        objects
            .map(|object| {
                if let Some(create_object_queue_item) = object.create_object_queue_item(
                    CloudObjectEventEntrypoint::default(),
                    InitiatedBy::User,
                ) {
                    create_object_queue_item
                } else {
                    object.update_object_queue_item(None)
                }
            })
            .collect::<Vec<_>>()
    }

    pub fn from_unsynced_actions(
        actions: impl Iterator<Item = (CloudObjectTypeAndId, ObjectAction)>,
    ) -> Vec<QueueItem> {
        actions
            .filter_map(|(id_and_type, action)| match action.action_subtype {
                ObjectActionSubtype::SingleAction {
                    timestamp,
                    data,
                    pending: true,
                    ..
                } => Some(QueueItem::RecordObjectAction {
                    id_and_type,
                    action_type: action.action_type,
                    action_timestamp: timestamp,
                    data,
                }),
                _ => None,
            })
            .collect::<Vec<_>>()
    }
}

/// `SyncQueue`'s event type. yarp never emits any event because no sync work
/// runs; this enum is intentionally empty.
#[derive(Debug, Clone)]
pub enum SyncQueueEvent {}

/// Inert stand-in for the cloud sync queue. Every operation is a no-op.
pub struct SyncQueue;

impl SyncQueue {
    #[cfg(test)]
    pub fn mock(_ctx: &mut ModelContext<Self>) -> Self {
        Self
    }

    pub fn new(_queue_items: Vec<QueueItem>, _ctx: &mut ModelContext<Self>) -> Self {
        Self
    }

    pub fn is_dequeueing(&self) -> bool {
        false
    }

    pub fn stop_dequeueing(&mut self) {}

    pub fn start_dequeueing(&mut self, _ctx: &mut ModelContext<Self>) {}

    /// Used during logout. No-op; the queue is always empty.
    pub fn clear(&mut self) {}

    /// Drop the request on the floor — there's no backend to receive it.
    pub fn enqueue(&mut self, _item: QueueItem, _ctx: &mut ModelContext<Self>) -> QueueItemId {
        QueueItemId::new()
    }
}

impl Entity for SyncQueue {
    type Event = SyncQueueEvent;
}

impl SingletonEntity for SyncQueue {}
