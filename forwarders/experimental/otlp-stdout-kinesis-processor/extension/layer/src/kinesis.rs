use aws_sdk_kinesis::primitives::Blob;
use aws_sdk_kinesis::types::PutRecordsRequestEntry;
use lambda_extension::{tracing, Error};
use uuid::Uuid;

// Kinesis limit for a single record
pub const MAX_RECORD_SIZE_BYTES: usize = 1_048_576; // 1MB per record

#[derive(Default)]
pub struct KinesisBatch {
    pub records: Vec<PutRecordsRequestEntry>,
}

impl KinesisBatch {
    pub fn add_record(&mut self, record: String) -> Result<(), Error> {
        if record.len() > MAX_RECORD_SIZE_BYTES {
            tracing::warn!(
                "Record size {} bytes exceeds maximum size of {} bytes, skipping",
                record.len(),
                MAX_RECORD_SIZE_BYTES
            );
            return Ok(());
        }

        match PutRecordsRequestEntry::builder()
            .data(Blob::new(record))
            .partition_key(Uuid::new_v4().to_string())
            .build()
        {
            Ok(entry) => {
                self.records.push(entry);
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to build Kinesis record entry: {}", e);
                Err(Error::from(e))
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*; // Import items from outer module
    use uuid::Uuid;

    #[test]
    fn test_add_record_success() {
        let mut batch = KinesisBatch::default();
        let record_data = "{\"key\": \"value\"}".to_string();
        let result = batch.add_record(record_data.clone());

        assert!(result.is_ok());
        assert_eq!(batch.records.len(), 1);

        let entry = &batch.records[0];
        assert_eq!(entry.data.as_ref(), record_data.as_bytes());
        // Check if partition key is a valid UUID
        assert!(Uuid::parse_str(entry.partition_key()).is_ok());
    }

    #[test]
    fn test_add_record_too_large() {
        let mut batch = KinesisBatch::default();
        // Create a string larger than the limit (e.g., 1MB + 1 byte)
        let large_record_data = "a".repeat(MAX_RECORD_SIZE_BYTES + 1);
        let result = batch.add_record(large_record_data);

        // Should succeed logically (record is skipped), but batch remains empty
        assert!(result.is_ok());
        assert!(batch.records.is_empty());
        // Ideally, capture logs to verify the warning was logged, but that's harder in basic unit tests.
    }

    #[test]
    fn test_add_record_at_limit() {
        let mut batch = KinesisBatch::default();
        // Create a string exactly at the limit
        let limit_record_data = "a".repeat(MAX_RECORD_SIZE_BYTES);
        let result = batch.add_record(limit_record_data.clone());

        assert!(result.is_ok());
        assert_eq!(batch.records.len(), 1);
        let entry = &batch.records[0];
        assert_eq!(entry.data.as_ref(), limit_record_data.as_bytes());
    }

    #[test]
    fn test_clear_batch() {
        let mut batch = KinesisBatch::default();
        batch.add_record("record1".to_string()).unwrap();
        batch.add_record("record2".to_string()).unwrap();
        assert!(!batch.is_empty());

        batch.clear();
        assert!(batch.is_empty());
    }
}
