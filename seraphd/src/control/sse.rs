use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};
use tokio_stream::StreamExt;
use crate::state::AppState;

pub async fn get_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| {
        match res {
            Ok(event) => {
                let json_str = serde_json::to_string(&event).unwrap();
                Some(Ok(Event::default().data(json_str)))
            }
            Err(BroadcastStreamRecvError::Lagged(_)) => {
                tracing::warn!("SSE stream lagged behind event bus");
                None
            }
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default().interval(Duration::from_secs(15)))
}
