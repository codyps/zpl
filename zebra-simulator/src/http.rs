use crate::{Mode, Simulator, MAX_REQUEST_BYTES};
use axum::{
    body::Bytes,
    extract::{
        rejection::{BytesRejection, FormRejection},
        DefaultBodyLimit, Path, Query, State,
    },
    http::{header, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse, Response,
    },
    routing::{get, post},
    Form, Json, Router,
};
use serde::Deserialize;
use std::convert::Infallible;

type HttpResult<T> = Result<T, (StatusCode, &'static str)>;
fn internal(_: eyre::Report) -> (StatusCode, &'static str) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "simulator storage operation failed",
    )
}

pub fn router(app: Simulator) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("../assets/index.html")) }),
        )
        .route("/api/requests", get(history))
        .route("/api/requests/{id}", get(detail))
        .route("/api/requests/{id}/raw", get(raw))
        .route("/api/requests/{id}/labels/{page}", get(image))
        .route("/api/events", get(events))
        .route("/pstprnt", post(print))
        .route("/zpl", post(preview))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .with_state(app)
}
#[derive(Deserialize)]
struct HistoryQuery {
    after: Option<i64>,
    before: Option<i64>,
}
async fn history(
    State(app): State<Simulator>,
    Query(query): Query<HistoryQuery>,
) -> HttpResult<Json<Vec<crate::Entry>>> {
    Ok(Json(
        app.history(query.after, query.before)
            .await
            .map_err(internal)?,
    ))
}
async fn detail(
    State(app): State<Simulator>,
    Path(id): Path<i64>,
) -> HttpResult<Json<crate::Detail>> {
    Ok(Json(
        app.detail(id)
            .await
            .map_err(internal)?
            .ok_or((StatusCode::NOT_FOUND, "request not found"))?,
    ))
}
async fn raw(State(app): State<Simulator>, Path(id): Path<i64>) -> HttpResult<Response> {
    let detail = app
        .detail(id)
        .await
        .map_err(internal)?
        .ok_or((StatusCode::NOT_FOUND, "request not found"))?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=request.bin",
            ),
        ],
        detail.raw,
    )
        .into_response())
}
async fn image(
    State(app): State<Simulator>,
    Path((id, page)): Path<(i64, i32)>,
) -> HttpResult<Response> {
    let png = app
        .image(id, page)
        .await
        .map_err(internal)?
        .ok_or((StatusCode::NOT_FOUND, "label not found"))?;
    Ok(([(header::CONTENT_TYPE, "image/png")], png).into_response())
}
async fn print(
    State(app): State<Simulator>,
    body: Result<Bytes, BytesRejection>,
) -> HttpResult<Response> {
    let body = match body {
        Ok(body) => body,
        Err(rejection) => {
            app.submit(
                Vec::new(),
                "http-print".into(),
                app.connection_id(),
                Mode::Invalid(format!("HTTP body rejected ({})", rejection.status())),
            )
            .await
            .map_err(internal)?;
            return Ok(rejection.into_response());
        }
    };
    let outcome = app
        .submit(
            body.to_vec(),
            "http-print".into(),
            app.connection_id(),
            Mode::Auto,
        )
        .await
        .map_err(internal)?;
    Ok(match outcome.error {
        Some(error) => (StatusCode::UNPROCESSABLE_ENTITY, error).into_response(),
        None => outcome.response.into_response(),
    })
}
#[derive(Deserialize)]
struct PreviewForm {
    data: String,
}
async fn preview(
    State(app): State<Simulator>,
    form: Result<Form<PreviewForm>, FormRejection>,
) -> HttpResult<Response> {
    let Form(form) = match form {
        Ok(form) => form,
        Err(rejection) => {
            app.submit(
                Vec::new(),
                "http-preview".into(),
                app.connection_id(),
                Mode::Invalid(format!(
                    "HTTP preview form rejected ({})",
                    rejection.status()
                )),
            )
            .await
            .map_err(internal)?;
            return Ok(rejection.into_response());
        }
    };
    // Zebra's form contains credentials, object name, and other metadata. Keep
    // only the submitted printer data; credentials are never stored in history.
    let outcome = app
        .submit(
            form.data.into_bytes(),
            "http-preview".into(),
            app.connection_id(),
            Mode::Preview,
        )
        .await
        .map_err(internal)?;
    Ok(match outcome.error {
        Some(error) => (StatusCode::UNPROCESSABLE_ENTITY, error).into_response(),
        None if outcome.pages > 0 => Html(format!(
            "<HTML><BODY><IMG SRC=\"/api/requests/{}/labels/0\"></BODY></HTML>",
            outcome.id
        ))
        .into_response(),
        None => (StatusCode::UNPROCESSABLE_ENTITY, "no label preview").into_response(),
    })
}
async fn events(State(app): State<Simulator>) -> impl IntoResponse {
    // Subscribe before taking the first snapshot. Watch coalesces updates; the
    // database cursor API recovers every row after bursts and reconnects.
    let receiver = app.updates.subscribe();
    let stream =
        futures_util::stream::unfold((receiver, true), |(mut receiver, first)| async move {
            if !first && receiver.changed().await.is_err() {
                return None;
            }
            let id = *receiver.borrow_and_update();
            Some((
                Ok::<_, Infallible>(
                    Event::default()
                        .event("requests")
                        .id(id.to_string())
                        .data(id.to_string()),
                ),
                (receiver, false),
            ))
        });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
