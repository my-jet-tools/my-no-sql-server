use std::{collections::VecDeque, time::Duration};

use my_http_server::HttpFailResult;
use my_no_sql_sdk::core::rust_extensions::{
    date_time::DateTimeAsMicroseconds, TaskCompletion, TaskCompletionAwaiter,
};

pub enum HttpPayload {
    Ping,
    Payload(Vec<u8>),
}

pub struct AwaitingResponse {
    pub created: DateTimeAsMicroseconds,
    task_completion: TaskCompletion<HttpPayload, HttpFailResult>,
}

pub struct HttpConnectionDeliveryInfo {
    awaiting_response: Option<AwaitingResponse>,
    payload_to_deliver: VecDeque<Vec<u8>>,
    id: String,
}
static MIN_PING_TIMEOUT: Duration = Duration::from_secs(3);

impl HttpConnectionDeliveryInfo {
    pub fn new(id: String) -> Self {
        Self {
            awaiting_response: None,
            payload_to_deliver: VecDeque::new(),
            id,
        }
    }

    pub fn upload(&mut self, payload: Vec<u8>) {
        match self.payload_to_deliver.pop_back() {
            Some(mut last_one) => {
                last_one.extend(payload);
                self.payload_to_deliver.push_back(last_one);
            }
            None => {
                self.payload_to_deliver.push_back(payload);
            }
        }
    }

    pub fn get_payload_to_deliver(&mut self) -> Option<Vec<u8>> {
        self.payload_to_deliver.pop_front()
    }

    pub fn ping(&mut self, now: DateTimeAsMicroseconds) {
        let ping_me = if let Some(item) = &self.awaiting_response {
            now.duration_since(item.created).as_positive_or_zero() >= MIN_PING_TIMEOUT
        } else {
            false
        };

        if !ping_me {
            return;
        }

        if let Some(mut task) = self.get_task_to_write_response() {
            if let Err(err) = task.try_set_ok(HttpPayload::Ping) {
                println!(
                    "Could not set ping result to http connection {}. Err:{:?}",
                    self.id, err
                );
            }
        }
    }

    pub fn get_task_to_write_response(
        &mut self,
    ) -> Option<TaskCompletion<HttpPayload, HttpFailResult>> {
        if self.awaiting_response.is_none() {
            return None;
        }

        let mut result = None;
        std::mem::swap(&mut self.awaiting_response, &mut result);
        let result = result?;

        result.task_completion.into()
    }

    pub fn issue_task_completion(&mut self) -> TaskCompletionAwaiter<HttpPayload, HttpFailResult> {
        if self.awaiting_response.is_some() {
            panic!("Task completion is already issued");
        }

        let mut task_completion = TaskCompletion::new();

        let result = task_completion.get_awaiter();

        let awaiting_response = AwaitingResponse {
            created: DateTimeAsMicroseconds::now(),
            task_completion,
        };

        self.awaiting_response = Some(awaiting_response);

        result
    }

    pub fn get_size(&self) -> usize {
        let mut result = 0;

        for delivery_info in &self.payload_to_deliver {
            result += delivery_info.len();
        }

        result
    }
}
