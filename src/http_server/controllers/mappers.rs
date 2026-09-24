use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;

pub trait ToSetExpirationTime {
    fn to_set_expiration_time(&self) -> Option<Option<DateTimeAsMicroseconds>>;
}

impl ToSetExpirationTime for Option<String> {
    fn to_set_expiration_time(&self) -> Option<Option<DateTimeAsMicroseconds>> {
        if let Some(dt) = self {
            Some(DateTimeAsMicroseconds::from_str(dt))
        } else {
            None
        }
    }
}
