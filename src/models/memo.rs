use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublicMemo {
    pub(crate) name: String,
    pub(crate) id: i64,
    pub(crate) uid: String,
    pub(crate) creator: MemoCreator,
    pub(crate) created_ts: i64,
    pub(crate) updated_ts: i64,
    pub(crate) row_status: String,
    pub(crate) content: String,
    pub(crate) visibility: String,
    pub(crate) pinned: bool,
    pub(crate) payload: Value,
    pub(crate) attachments: Vec<Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub(crate) struct MemoCreator {
    pub(crate) id: i64,
    pub(crate) username: String,
    pub(crate) nickname: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbMemo {
    pub(crate) id: i64,
    pub(crate) uid: String,
    pub(crate) creator_id: i64,
    pub(crate) creator_username: String,
    pub(crate) creator_nickname: String,
    pub(crate) created_ts: i64,
    pub(crate) updated_ts: i64,
    pub(crate) row_status: String,
    pub(crate) content: String,
    pub(crate) visibility: String,
    pub(crate) pinned: i64,
    pub(crate) payload: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbMemoRelation {
    pub(crate) uid: String,
    pub(crate) content: String,
    #[serde(rename = "type")]
    pub(crate) relation_type: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbMemoEvent {
    pub(crate) id: i64,
    pub(crate) event_type: String,
    pub(crate) name: String,
    pub(crate) visibility: String,
    pub(crate) creator_id: i64,
    pub(crate) payload: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct RelationCandidate {
    pub(crate) uid: String,
    pub(crate) content: String,
    pub(crate) payload: String,
    pub(crate) updated_ts: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct RankedRelationCandidate {
    pub(crate) uid: String,
    pub(crate) content: String,
    pub(crate) score: f64,
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MemoChildRoute {
    ListComments,
    CreateComment,
    GetRelations,
    SuggestRelations,
    SetRelations,
    Unsupported,
}
