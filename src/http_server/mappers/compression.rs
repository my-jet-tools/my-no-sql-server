use my_http_server::{HttpOkResult, HttpOutput};

const ZSTD_LEVEL: i32 = 11;
const RESPONSE_HEADER: &str = "x-content-encoding";
pub const COMPRESSION_THRESHOLD: usize = 4 * 1024;

pub fn wants_zstd(header_value: Option<&str>) -> bool {
    let Some(v) = header_value else {
        return false;
    };
    v.to_ascii_lowercase().contains("zstd")
}

pub fn try_compress_zstd(mut result: HttpOkResult, threshold: usize) -> HttpOkResult {
    let (content, headers) = match &mut result.output {
        HttpOutput::Content {
            content, headers, ..
        } => (content, headers),
        HttpOutput::File {
            content, headers, ..
        } => (content, headers),
        _ => return result,
    };

    if content.len() <= threshold {
        return result;
    }

    let Ok(compressed) = zstd::encode_all(content.as_slice(), ZSTD_LEVEL) else {
        return result;
    };

    if compressed.len() * 100 > content.len() * 80 {
        return result;
    }

    *content = compressed;
    headers.add_header(RESPONSE_HEADER.into(), "zstd".to_string());
    result
}
