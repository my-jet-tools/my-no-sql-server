use std::{sync::Arc, time::Duration};

use crate::app::AppContext;

pub async fn start(app: Arc<AppContext>) {
    let timeout = Duration::from_secs(60);

    while !app.states.is_shutting_down() {
        tokio::time::sleep(timeout).await;

        let result = tokio::spawn(detect_dead_connections(app.clone(), timeout)).await;

        if let Err(err) = result {
            app.logs
                .add_fatal_error(
                    crate::app::logs::SystemProcess::TcpSocket,
                    "dead_connections_detector".to_string(),
                    format!("{}", err),
                )
                .await;
        }
    }
}

pub async fn detect_dead_connections(app: Arc<AppContext>, timeout: Duration) {
    let dead_sessions = app.data_readers.get_dead_connections(timeout).await;

    if let Some(dead_sessions) = dead_sessions {
        for dead_session in dead_sessions {
            crate::operations::sessions::disconnect(dead_session).await;
        }
    }
}
