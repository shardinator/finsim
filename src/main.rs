mod models;
mod storage;

use std::path::PathBuf;
use std::sync::Arc;

use models::Bank;
use poem::{
    endpoint::StaticFilesEndpoint,
    get, handler, post,
    http::{header::CONTENT_TYPE, StatusCode},
    listener::TcpListener,
    web::{Data, Form, Path, Redirect},
    EndpointExt, Response, Route, Server,
};
use serde::Deserialize;
use tera::{Context, Tera};
use tokio::sync::Mutex;

#[derive(Clone)]
struct AppState {
    tera: Tera,
    banks: Arc<Mutex<Vec<Bank>>>,
    storage_path: PathBuf,
}

#[derive(Deserialize)]
struct CreateBankForm {
    name: String,
}

#[derive(Deserialize)]
struct BankIdPath {
    id: u64,
}

#[handler]
async fn home(state: Data<&AppState>) -> poem::Result<Response> {
    let banks = state.banks.lock().await.clone();
    let mut ctx = Context::new();
    ctx.insert("banks", &banks);

    let html = state.tera.render("home.html", &ctx).map_err(|e| {
        poem::Error::from_string(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    Ok(Response::builder()
        .header(CONTENT_TYPE, "text/html; charset=utf-8")
        .body(html))
}

#[handler]
async fn create_bank(
    state: Data<&AppState>,
    Form(form): Form<CreateBankForm>,
) -> poem::Result<Redirect> {
    let name = form.name.trim();
    if name.is_empty() {
        return Ok(Redirect::see_other("/"));
    }

    let mut banks = state.banks.lock().await;
    let next_id = banks
        .iter()
        .map(|b| b.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    banks.push(Bank::new(next_id, name.to_string()));

    match storage::save_banks(&state.storage_path, &banks) {
        Ok(()) => Ok(Redirect::see_other("/")),
        Err(e) => {
            banks.pop();
            Err(poem::Error::from_string(
                format!("failed to save banks: {e}"),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

#[handler]
async fn remove_bank(
    state: Data<&AppState>,
    Path(BankIdPath { id }): Path<BankIdPath>,
) -> poem::Result<Redirect> {
    let mut banks = state.banks.lock().await;
    let backup = banks.clone();
    banks.retain(|b| b.id != id);
    if banks.len() == backup.len() {
        return Ok(Redirect::see_other("/"));
    }

    match storage::save_banks(&state.storage_path, &banks) {
        Ok(()) => Ok(Redirect::see_other("/")),
        Err(e) => {
            *banks = backup;
            Err(poem::Error::from_string(
                format!("failed to save banks: {e}"),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let tera = Tera::new("templates/**/*").expect("failed to load templates");
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);

    let storage_path = storage::banks_file_path();
    let initial_banks = storage::load_banks(&storage_path);

    let state = AppState {
        tera,
        banks: Arc::new(Mutex::new(initial_banks)),
        storage_path,
    };

    let app = Route::new()
        .nest("/images", StaticFilesEndpoint::new("images"))
        .at("/", get(home))
        .at("/banks/:id/remove", post(remove_bank))
        .at("/banks", post(create_bank))
        .data(state);

    Server::new(TcpListener::bind(("0.0.0.0", port)))
        .run(app)
        .await
}
