use yarpui::AppContext;

use crate::server::cloud_objects::update_manager::{
    InitiatedBy, ObjectOperation, OperationSuccessType,
};

use super::{CloudObject, GenericStringObjectFormat, JsonObjectType, ObjectType};

pub struct CloudObjectToastMessage;

impl CloudObjectToastMessage {
    pub fn toast_message(
        object: &dyn CloudObject,
        operation: &ObjectOperation,
        success_type: &OperationSuccessType,
        app: &AppContext,
    ) -> Option<String> {
        let object_name = object.model_type_name().to_owned();
        let object_name_lowercase = object_name.to_ascii_lowercase();

        match (object.object_type(), operation, success_type) {
            // We should only show toasts for creates initiated by the user, not by the system
            (_, ObjectOperation::Create { initiated_by: InitiatedBy::User }, OperationSuccessType::Success) => {
                let containing_object_name = object.containing_object_name(app);
                Some(format!("{object_name} filed under {containing_object_name}"))
            }
            // notebooks intentionally do not have an update message, as they are updated
            // as the user types and so toasts would be VERY noisy
            (
                ObjectType::Notebook,
                ObjectOperation::Update,
                OperationSuccessType::Success,
            ) => None,
            (_, ObjectOperation::Update, OperationSuccessType::Success) => {
                Some(format!("{object_name} amended"))
            }
            (_, ObjectOperation::MoveToFolder, OperationSuccessType::Success) | (_, ObjectOperation::MoveToDrive, OperationSuccessType::Success) => {
                let containing_object_name = object.containing_object_name(app);
                Some(format!("{object_name} transferred to {containing_object_name}"))
            }
            (_, ObjectOperation::Trash, OperationSuccessType::Success) => {
                Some(format!("{object_name} bagged into the evidence locker"))
            }
            (_, ObjectOperation::Untrash, OperationSuccessType::Success) => {
                Some(format!("{object_name} pulled from the evidence locker"))
            }
            (_, ObjectOperation::Leave, OperationSuccessType::Success) => {
                Some(format!("Off the {object_name_lowercase} squad"))
            }
            (_, ObjectOperation::Create { initiated_by: InitiatedBy::User }, OperationSuccessType::Failure) => {
                Some(format!("Couldn't open the {object_name_lowercase}"))
            }
            (_, ObjectOperation::Create { initiated_by: InitiatedBy::User }, OperationSuccessType::Denied(message)) => {
                Some(message.to_string())
            }
            (_, ObjectOperation::Update, OperationSuccessType::Failure) => {
                Some(format!("Couldn't amend the {object_name_lowercase}"))
            }
            (_, ObjectOperation::MoveToFolder, OperationSuccessType::Failure) | (_, ObjectOperation::MoveToDrive, OperationSuccessType::Failure) => {
                Some(format!("Couldn't transfer the {object_name_lowercase}"))
            }
            (_, ObjectOperation::Trash, OperationSuccessType::Failure) => {
                Some(format!("Couldn't bag the {object_name_lowercase}"))
            }
            (_, ObjectOperation::Untrash, OperationSuccessType::Failure) => {
                Some(format!("Couldn't pull the {object_name_lowercase} from the evidence locker"))
            }
            // We should only show deletion failure toasts for user-initiated deletions.
            (_, ObjectOperation::Delete { initiated_by: InitiatedBy::User }, OperationSuccessType::Failure) => {
                Some(format!("Couldn't strike the {object_name_lowercase} from the books"))
            }
            (_, ObjectOperation::Leave, OperationSuccessType::Failure) => {
                Some(format!("Couldn't sign off the {object_name_lowercase}"))
            }
            (
                ObjectType::Workflow,
                ObjectOperation::Update,
                OperationSuccessType::Rejection,
            ) => {
                Some("Couldn't file this casebook — someone else amended it while you were on the books.".to_string())
            }
            (
                ObjectType::GenericStringObject(GenericStringObjectFormat::Json(JsonObjectType::EnvVarCollection)),
                ObjectOperation::Update,
                OperationSuccessType::Rejection,
            ) => {
                Some("Couldn't file the environment variables — someone else amended them while you were on the books.".to_string())
            }
            (
                ObjectType::GenericStringObject(GenericStringObjectFormat::Json(JsonObjectType::AIFact)),
                ObjectOperation::Update,
                OperationSuccessType::Rejection,
            ) => {
                Some("Couldn't file the rule — someone else amended it while you were on the books.".to_string())
            }
            (_, ObjectOperation::TakeEditAccess, OperationSuccessType::Failure) => {
                Some(format!("Couldn't get clearance to amend the {object_name_lowercase}"))
            }
            (_, ObjectOperation::UpdatePermissions, OperationSuccessType::Success) => {
                Some(format!("Clearances amended for {object_name_lowercase}"))
            }
            (_, ObjectOperation::UpdatePermissions, OperationSuccessType::Failure) => {
                Some(format!("Couldn't amend clearances for {object_name_lowercase}"))
            }
            _ => None,
        }
    }

    pub fn toast_deletion_confirm_message(
        num_objects: i32,
        operation: &ObjectOperation,
        success_type: &OperationSuccessType,
    ) -> Option<String> {
        let count_objects_message = match num_objects {
            1 => "1 case file".to_string(),
            n => {
                format!("{n} case files")
            }
        };
        match (operation, success_type) {
            // We should only show deletion failure toasts for user-initiated deletions.
            (
                ObjectOperation::Delete {
                    initiated_by: InitiatedBy::User,
                },
                OperationSuccessType::Success,
            ) => Some(format!("{count_objects_message} incinerated")),
            (ObjectOperation::EmptyTrash, OperationSuccessType::Success) => Some(format!(
                "Evidence locker incinerated: {count_objects_message} burned"
            )),
            (ObjectOperation::EmptyTrash, OperationSuccessType::Failure) => {
                Some("Couldn't incinerate the evidence locker.".to_string())
            }
            (ObjectOperation::EmptyTrash, OperationSuccessType::Rejection) => {
                Some("Evidence locker's already empty.".to_string())
            }
            _ => None,
        }
    }
}
