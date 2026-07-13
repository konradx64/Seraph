use crate::state::AppState;
use axum::{
    extract::State,
    response::sse::{Event as SseEvent, KeepAlive, Sse},
};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};
use tokio_stream::{StreamExt, once};

pub async fn get_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<SseEvent, Infallible>>> {
    state.stats.flush_events();

    let snapshot = crate::event::Event::DashboardSnapshot {
        routes: crate::control::routes::route_snapshot(&state),
        certs: crate::control::certs::cert_snapshot(&state).unwrap_or_else(|e| {
            tracing::error!("Failed to load certificates for dashboard snapshot: {}", e);
            Vec::new()
        }),
        tunnels: crate::control::tunnels::tunnel_snapshot(&state)
            .await
            .unwrap_or_else(|e| {
                tracing::error!("Failed to load tunnels for dashboard snapshot: {}", e);
                Vec::new()
            }),
        status: crate::control::tunnels::status_snapshot(&state),
        stats: state.stats.get_snapshot(),
    };
    let snapshot_stream = once(Ok(serialize_sse_event(snapshot)));

    let rx = state.events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| match res {
        Ok(event) => Some(Ok(serialize_sse_event(event))),
        Err(BroadcastStreamRecvError::Lagged(_)) => {
            tracing::warn!("SSE stream lagged behind event bus");
            None
        }
    });

    Sse::new(snapshot_stream.chain(stream))
        .keep_alive(KeepAlive::default().interval(Duration::from_secs(15)))
}

fn serialize_sse_event(event: crate::event::Event) -> SseEvent {
    let json_str = serde_json::to_string(&event).unwrap();
    SseEvent::default().data(json_str)
}
