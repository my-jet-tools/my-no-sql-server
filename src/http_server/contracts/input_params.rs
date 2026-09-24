use crate::db_sync::DataSynchronizationPeriod;

pub const PARAM_TABLE_NAME: &str = "tableName";

pub const DEFAULT_SYNC_PERIOD: DataSynchronizationPeriod = DataSynchronizationPeriod::Sec5;
