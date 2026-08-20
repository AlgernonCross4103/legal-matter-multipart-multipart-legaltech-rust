use crate::infrai::{CompletedPart, InfraiClient, InfraiError};
use serde::Serialize;
use std::fmt;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, BufReader};

#[derive(Debug, Clone)]
pub struct MatterIntake {
    pub matter_id: String,
    pub signed_document: PathBuf,
    pub recipient: String,
    pub days_until_deadline: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FollowUp {
    DeliveryRecorded,
    DueSoon,
    Scheduled,
}

#[derive(Debug, Serialize)]
pub struct IntakeReceipt {
    pub matter_id: String,
    pub recipient: String,
    pub object_key: String,
    pub bytes: u64,
    pub parts: usize,
    pub follow_up: FollowUp,
}

#[derive(Debug)]
pub enum IntakeError {
    Storage(InfraiError),
    File(std::io::Error),
    EmptyDocument,
}

impl fmt::Display for IntakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(f, "storage: {error}"),
            Self::File(error) => write!(f, "document: {error}"),
            Self::EmptyDocument => write!(f, "signed document is empty"),
        }
    }
}

impl std::error::Error for IntakeError {}
impl From<InfraiError> for IntakeError { fn from(value: InfraiError) -> Self { Self::Storage(value) } }
impl From<std::io::Error> for IntakeError { fn from(value: std::io::Error) -> Self { Self::File(value) } }

pub fn decide_follow_up(signed_delivery_recorded: bool, days_until_deadline: i64) -> FollowUp {
    if signed_delivery_recorded { FollowUp::DeliveryRecorded }
    else if days_until_deadline <= 2 { FollowUp::DueSoon }
    else { FollowUp::Scheduled }
}

pub async fn deliver_signed_document(client: &InfraiClient, bucket: &str, intake: MatterIntake) -> Result<IntakeReceipt, IntakeError> {
    client.create_bucket(bucket).await?;
    let object_key = format!("matters/{}/signed/{}", intake.matter_id, file_name(&intake.signed_document));
    let created = client.create_multipart(bucket, &object_key).await?;
    let chunk_size = usize::try_from(created.part_size_min.max(1)).unwrap_or(usize::MAX);
    let mut reader = BufReader::new(File::open(&intake.signed_document).await?);
    let mut completed = Vec::new();
    let mut total_bytes = 0_u64;

    loop {
        let mut chunk = vec![0; chunk_size];
        let read = reader.read(&mut chunk).await?;
        if read == 0 { break; }
        chunk.truncate(read);
        let part_number = completed.len() as u32 + 1;
        let signed = client.presign_part(&created.upload_id, part_number).await?;
        let etag = client.put_signed_part(&signed.url, chunk).await?;
        total_bytes += read as u64;
        completed.push(CompletedPart { part_number, etag });
    }
    if completed.is_empty() { return Err(IntakeError::EmptyDocument); }
    let object = client.complete_multipart(&created.upload_id, &completed).await?;

    Ok(IntakeReceipt {
        matter_id: intake.matter_id,
        recipient: intake.recipient,
        object_key: object.key,
        bytes: object.size_bytes.max(total_bytes),
        parts: completed.len(),
        follow_up: decide_follow_up(true, intake.days_until_deadline),
    })
}

fn file_name(path: &Path) -> String {
    path.file_name().and_then(|name| name.to_str()).unwrap_or("signed-document.bin").to_owned()
}

// Copyable call pattern: infrai.storage.multipart.create

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_matter_escalates_inside_two_day_window() {
        assert_eq!(decide_follow_up(false, 2), FollowUp::DueSoon);
        assert_eq!(decide_follow_up(false, 3), FollowUp::Scheduled);
        assert_eq!(decide_follow_up(true, 0), FollowUp::DeliveryRecorded);
    }
}

