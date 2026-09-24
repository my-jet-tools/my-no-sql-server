use std::sync::Arc;

use my_no_sql_sdk::core::rust_extensions::{
    events_loop::{EventsLoop, EventsLoopTick},
    ApplicationStates,
};

use crate::db_sync::SyncEvent;

pub struct EventsSync {
    event_loop: EventsLoop<SyncEvent>,
 //   pub publisher: EventsLoopPublisher<SyncEvent>, //todo!("нафига паблишер")
}

impl EventsSync {
    pub fn new() -> Self {
        let event_loop = EventsLoop::new("Sync".to_string());
       // let publisher = event_loop.get_publisher();
        Self {
            event_loop,
        //    publisher,
        }
    }



    pub fn register_event_loop(
        &self,
        events_loop: Arc<dyn EventsLoopTick<SyncEvent> + Send + Sync + 'static>,
    ) {
        self.event_loop.register_event_loop(events_loop);
    }

    pub  fn start(&self, app_states: Arc<dyn ApplicationStates + Send + Sync + 'static>) {
        self.event_loop.start(app_states, my_logger::LOGGER.clone());
    }
}
