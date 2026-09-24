use std::sync::atomic::AtomicUsize;

use my_http_server::HttpFailResult;
use my_no_sql_sdk::core::rust_extensions::date_time::{
    AtomicDateTimeAsMicroseconds, DateTimeAsMicroseconds,
};
use tokio::sync::Mutex;

use crate::{
    data_readers::http_connection::connection_delivery_info::HttpPayload, db_sync::SyncEvent,
};

use super::{into_http_payload, HttpConnectionDeliveryInfo};

pub struct HttpConnectionInfo {
    pub id: String,
    pub ip: String,
    pub connected: DateTimeAsMicroseconds,
    pub last_incoming_moment: AtomicDateTimeAsMicroseconds,
    pub delivery_info: Mutex<HttpConnectionDeliveryInfo>,
    pending_to_send: AtomicUsize,
    name: String,
    #[allow(dead_code)]
    version: String,
}

impl HttpConnectionInfo {
    pub fn new(id: String, name: String, version: String, ip: String) -> Self {
        Self {
            id: id.to_string(),
            ip,
            connected: DateTimeAsMicroseconds::now(),
            last_incoming_moment: AtomicDateTimeAsMicroseconds::now(),
            delivery_info: Mutex::new(HttpConnectionDeliveryInfo::new(id)),
            pending_to_send: AtomicUsize::new(0),
            name,
            version,
        }
    }

    pub fn get_name(&self) -> &str {
        self.name.as_str()
    }

    pub async fn ping(&self, now: DateTimeAsMicroseconds) {
        let mut delivery_info = self.delivery_info.lock().await;
        delivery_info.ping(now)
    }

    pub async fn send(&self, sync_event: &SyncEvent) {
        if let Some(payload) = into_http_payload::convert(sync_event).await {
            let mut delivery_info_write_access = self.delivery_info.lock().await;
            delivery_info_write_access.upload(payload);
            self.pending_to_send.store(
                delivery_info_write_access.get_size(),
                std::sync::atomic::Ordering::SeqCst,
            );

            if let Some(mut task) = delivery_info_write_access.get_task_to_write_response() {
                let payload = delivery_info_write_access.get_payload_to_deliver().unwrap();

                if let Err(err) = task.try_set_ok(HttpPayload::Payload(payload)) {
                    println!(
                        "Sending payload Error for the session: {}. Reason:{:?}",
                        self.id, err
                    );
                }
            }
        }
    }

    pub async fn new_request(&self) -> Result<HttpPayload, HttpFailResult> {
        let task_completion = {
            let mut write_access = self.delivery_info.lock().await;

            if let Some(payload) = write_access.get_payload_to_deliver() {
                return Ok(HttpPayload::Payload(payload));
            }

            write_access.issue_task_completion()
        };

        task_completion.get_result().await
    }

    pub fn get_pending_to_send(&self) -> usize {
        self.pending_to_send
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}
