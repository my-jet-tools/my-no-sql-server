pub enum ReadOperationResult {
    SingleRow(Vec<u8>),
    RowsArray(Vec<u8>),
    EmptyArray,
}
