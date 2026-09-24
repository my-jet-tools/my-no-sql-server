use my_http_server::FileContent;

/// Local newtype over `FileContent`.
///
/// `MyHttpInput` emits, for any struct-typed `#[http_form_data]` field, a schema call
/// (`get_data_type`) and a client-side `as_str()`. my-http-utils 0.1.0 dropped the
/// `FileContent` special case that used to skip both, and the orphan rule forbids adding
/// them to the foreign type — so the field carries this wrapper instead.
pub struct UploadedFile(pub FileContent);

impl UploadedFile {
    pub fn into_content(self) -> Vec<u8> {
        self.0.content
    }

    /// Never called on the server; a file part is written by the multipart body builder, not
    /// as a string. Present only because the derive emits a call to it for the client side.
    pub fn as_str(&self) -> &str {
        self.0.file_name.as_str()
    }
}

impl my_http_server::controllers::documentation::DataTypeProvider for UploadedFile {
    fn get_data_type() -> my_http_server::controllers::documentation::data_types::HttpDataType {
        my_http_server::controllers::documentation::data_types::HttpDataType::SimpleType(
            my_http_server::controllers::documentation::data_types::HttpSimpleType::Binary,
        )
    }
}

impl<'s> TryFrom<my_http_utils::http_input::HttpInputValue<'s>> for UploadedFile {
    type Error = my_http_utils::http_input::HttpParseError;

    fn try_from(value: my_http_utils::http_input::HttpInputValue<'s>) -> Result<Self, Self::Error> {
        let content: FileContent = value.try_into()?;
        Ok(Self(content))
    }
}
