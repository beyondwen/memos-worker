use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbAttachment {
    pub(crate) id: i64,
    pub(crate) uid: String,
    pub(crate) creator_id: i64,
    pub(crate) created_ts: i64,
    pub(crate) filename: String,
    #[serde(rename = "type")]
    pub(crate) file_type: String,
    pub(crate) size: i64,
    pub(crate) memo_id: Option<i64>,
    pub(crate) reference: String,
    pub(crate) memo_visibility: Option<String>,
    pub(crate) memo_creator_id: Option<i64>,
}
