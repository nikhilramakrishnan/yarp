use crate::{error::UserFacingError, schema};

#[derive(cynic::QueryVariables, Debug)]
pub struct ListYarpDevImagesVariables {}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "RootQuery", variables = "ListYarpDevImagesVariables")]
pub struct ListYarpDevImages {
    #[cynic(rename = "listYarpDevImages")]
    pub list_yarp_dev_images: ListYarpDevImagesResult,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
pub struct ListYarpDevImagesOutput {
    pub images: Vec<ImageTag>,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
pub struct ImageTag {
    pub image: String,
    pub repository: String,
    pub tag: String,
}

#[derive(cynic::InlineFragments, Debug)]
pub enum ListYarpDevImagesResult {
    ListYarpDevImagesOutput(ListYarpDevImagesOutput),
    UserFacingError(UserFacingError),
    #[cynic(fallback)]
    Unknown,
}

crate::client::define_operation! {
    ListYarpDevImages(ListYarpDevImagesVariables) -> ListYarpDevImages;
}
