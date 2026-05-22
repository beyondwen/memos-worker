#[derive(Debug, PartialEq)]
pub(crate) struct BackupArtifact {
    pub(crate) key: String,
    pub(crate) size: usize,
    pub(crate) encrypted: bool,
    pub(crate) key_id: Option<String>,
}
