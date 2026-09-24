use my_no_sql_sdk::server::rust_extensions::base64::FromBase64;

use crate::scripts::TABLE_METADATA_FILE_NAME;

pub enum ArchiveFileType {
    Metadata,
    PartitionKey(String),
}

impl ArchiveFileType {
    pub fn is_metadata(&self) -> bool {
        matches!(self, Self::Metadata)
    }

    pub fn unwrap_as_partition_key(self) -> String {
        match self {
            Self::PartitionKey(key) => key,
            _ => panic!("Can not unwrap partition key"),
        }
    }
}

pub struct RestoreFileName {
    pub table_name: String,
    pub file_type: ArchiveFileType,
    pub file_name: String,
}

impl RestoreFileName {
    pub fn new(file_name: &str) -> Result<Option<Self>, String> {
        let table_separator = file_name.find(std::path::MAIN_SEPARATOR);

        if table_separator.is_none() {
            return Ok(None);
        }

        let table_separator = table_separator.unwrap();

        let partition_key = &file_name[table_separator + 1..];

        if partition_key.is_empty() {
            return Ok(None);
        }

        if partition_key == TABLE_METADATA_FILE_NAME {
            return Ok(Self {
                table_name: file_name[..table_separator].to_string(),
                file_type: ArchiveFileType::Metadata,
                file_name: file_name.to_string(),
            }
            .into());
        }

        let partition_key = partition_key.from_base64();

        if partition_key.is_err() {
            return Err(format!(
                "Invalid file_name key [{}]. Can not extract partition key",
                file_name
            ));
        }

        let partition_key = partition_key.unwrap();

        let partition_key = match String::from_utf8(partition_key) {
            Ok(result) => result,
            Err(_) => {
                return Err(format!(
                    "Invalid file_name key [{}]. Can not convert partition key to string",
                    file_name
                ));
            }
        };

        let result = Self {
            table_name: file_name[..table_separator].to_string(),
            file_type: ArchiveFileType::PartitionKey(partition_key),
            file_name: file_name.to_string(),
        };

        Ok(result.into())
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_metadata_file() {
        let result = super::RestoreFileName::new("key-value/.metadata")
            .unwrap()
            .unwrap();
        assert_eq!(result.table_name, "key-value");
        assert!(result.file_type.is_metadata());
    }

    #[test]
    fn test_partition_key() {
        let result = super::RestoreFileName::new("key-value/Yw==")
            .unwrap()
            .unwrap();
        assert_eq!(result.table_name, "key-value");
        assert!(result.file_type.unwrap_as_partition_key() == "c");
    }

    #[test]
    fn test_empty_folder() {
        let result = super::RestoreFileName::new("key-value").unwrap();
        assert!(result.is_none());

        let result = super::RestoreFileName::new("key-value/").unwrap();
        assert!(result.is_none());
    }
}
