use crate::cloud_object::extract_server_id_and_object_type_from_yarp_drive_link;
use crate::drive::OpenYarpDriveObjectArgs;
use crate::ChannelState;
use url::Url;

#[derive(PartialEq, Debug)]
pub enum YarpWebLink {
    Session,
    DriveObject(Box<OpenYarpDriveObjectArgs>),
}

pub fn get_item_data_from_warp_link(url: &Url) -> Option<YarpWebLink> {
    if url.origin() == ChannelState::server_root_domain() {
        url.path_segments().and_then(|mut path_segments| {
            path_segments.next().and_then(|segment| match segment {
                "drive" => extract_server_id_and_object_type_from_yarp_drive_link(url)
                    .map(|args| YarpWebLink::DriveObject(Box::new(args))),
                "session" => Some(YarpWebLink::Session),
                _ => None,
            })
        })
    } else {
        None
    }
}
