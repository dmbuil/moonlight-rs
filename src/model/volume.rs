//! Cinder: volumes, snapshots, backups.

use super::*;

/// Where a volume is attached (server and device), names resolved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VolumeAttachment {
    pub server_id: String,
    pub server_name: Option<String>,
    pub device: Option<String>,
}

/// A row of the Volumes list (Cinder v3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Volume {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub size_gb: i64,
    pub status: String,
    pub volume_type: Option<String>,
    pub attachments: Vec<VolumeAttachment>,
    pub availability_zone: Option<String>,
    pub bootable: bool,
    pub encrypted: bool,
    /// Cinder volume metadata (roadmap 25).
    #[serde(default)]
    pub metadata: Vec<(String, String)>,
}

/// A row of the Volume Snapshots list (Cinder v3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub size_gb: i64,
    pub status: String,
    pub volume_id: String,
    pub volume_name: Option<String>,
    pub created: String,
    /// Cinder snapshot metadata (roadmap 25).
    #[serde(default)]
    pub metadata: Vec<(String, String)>,
}

/// A row of Volumes › Backups (Cinder, roadmap 5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BackupRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub size_gb: i64,
    /// The volume it was taken of; the name falls back to the ID (deleted or unlisted).
    pub volume: Ref,
    /// `true` for an incremental backup (`is_incremental`).
    pub incremental: bool,
    /// Swift container holding the data (`None` with other backends).
    pub container: Option<String>,
    /// Source snapshot, when the backup was made from one.
    pub snapshot_id: Option<String>,
    pub availability_zone: Option<String>,
    /// Why the backup failed (`fail_reason`), if it did.
    pub fail_reason: Option<String>,
    pub created: String,
}
