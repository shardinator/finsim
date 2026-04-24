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
    web::{Data, Form, Path, Query, Redirect},
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

#[derive(Deserialize)]
struct RemoveBankForm {
    confirmation: String,
}

#[derive(Deserialize)]
struct UiQuery {
    #[serde(default, rename = "removeError")]
    remove_error: Option<String>,
}

#[handler]
async fn settings(
    state: Data<&AppState>,
    Query(query): Query<UiQuery>,
) -> poem::Result<Response> {
    let banks = state.banks.lock().await.clone();
    let mut ctx = Context::new();
    ctx.insert("banks", &banks);
    ctx.insert("remove_error", &query.remove_error.is_some());

    let html = state.tera.render("settings.html", &ctx).map_err(|e| {
        poem::Error::from_string(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    Ok(Response::builder()
        .header(CONTENT_TYPE, "text/html; charset=utf-8")
        .body(html))
}

#[handler]
async fn home(
    state: Data<&AppState>,
    Query(_query): Query<UiQuery>,
) -> poem::Result<Response> {
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
        return Ok(Redirect::see_other("/settings"));
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
        Ok(()) => Ok(Redirect::see_other("/settings")),
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
async fn update_bank(
    state: Data<&AppState>,
    Path(BankIdPath { id }): Path<BankIdPath>,
    Form(form): Form<CreateBankForm>,
) -> poem::Result<Redirect> {
    let name = form.name.trim();
    if name.is_empty() {
        return Ok(Redirect::see_other("/settings"));
    }

    let mut banks = state.banks.lock().await;
    let backup = banks.clone();
    let mut found = false;
    for b in banks.iter_mut() {
        if b.id == id {
            b.name = name.to_string();
            found = true;
            break;
        }
    }
    if !found {
        return Ok(Redirect::see_other("/settings"));
    }

    match storage::save_banks(&state.storage_path, &banks) {
        Ok(()) => Ok(Redirect::see_other("/settings")),
        Err(e) => {
            *banks = backup;
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
    Form(form): Form<RemoveBankForm>,
) -> poem::Result<Redirect> {
    let typed = form.confirmation.trim();
    let mut banks = state.banks.lock().await;
    let bank_name = match banks.iter().find(|b| b.id == id) {
        Some(b) => b.name.clone(),
        None => return Ok(Redirect::see_other("/")),
    };
    let expected = format!("Please remove {}", bank_name);
    if typed != expected {
        return Ok(Redirect::see_other("/settings?removeError=1"));
    }

    let backup = banks.clone();
    banks.retain(|b| b.id != id);
    if banks.len() == backup.len() {
        return Ok(Redirect::see_other("/settings"));
    }

    match storage::save_banks(&state.storage_path, &banks) {
        Ok(()) => Ok(Redirect::see_other("/settings")),
        Err(e) => {
            *banks = backup;
            Err(poem::Error::from_string(
                format!("failed to save banks: {e}"),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

fn swap_banks_by_index(banks: &mut [Bank], a: usize, b: usize) {
    banks.swap(a, b);
}

#[handler]
async fn move_bank_up(
    state: Data<&AppState>,
    Path(BankIdPath { id }): Path<BankIdPath>,
) -> poem::Result<Redirect> {
    let mut banks = state.banks.lock().await;
    let backup = banks.clone();

    let Some(idx) = banks.iter().position(|b| b.id == id) else {
        return Ok(Redirect::see_other("/settings"));
    };
    if idx == 0 {
        return Ok(Redirect::see_other("/settings"));
    }

    swap_banks_by_index(&mut banks, idx, idx.saturating_sub(1));
    match storage::save_banks(&state.storage_path, &banks) {
        Ok(()) => Ok(Redirect::see_other("/settings")),
        Err(e) => {
            *banks = backup;
            Err(poem::Error::from_string(
                format!("failed to save banks: {e}"),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

#[handler]
async fn move_bank_down(
    state: Data<&AppState>,
    Path(BankIdPath { id }): Path<BankIdPath>,
) -> poem::Result<Redirect> {
    let mut banks = state.banks.lock().await;
    let backup = banks.clone();

    let Some(idx) = banks.iter().position(|b| b.id == id) else {
        return Ok(Redirect::see_other("/settings"));
    };
    if idx.saturating_add(1) >= banks.len() {
        return Ok(Redirect::see_other("/settings"));
    }

    swap_banks_by_index(&mut banks, idx, idx + 1);
    match storage::save_banks(&state.storage_path, &banks) {
        Ok(()) => Ok(Redirect::see_other("/settings")),
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

    eprintln!("FinSim: bank data path {}", storage_path.display());

    let state = AppState {
        tera,
        banks: Arc::new(Mutex::new(initial_banks)),
        storage_path,
    };

    let app = Route::new()
        .nest("/images", StaticFilesEndpoint::new("images"))
        .at("/", get(home))
        .at("/settings", get(settings))
        .at("/banks/:id/remove", post(remove_bank))
        .at("/banks/:id/update", post(update_bank))
        .at("/banks/:id/move-up", post(move_bank_up))
        .at("/banks/:id/move-down", post(move_bank_down))
        .at("/banks", post(create_bank))
        .data(state);

    Server::new(TcpListener::bind(("0.0.0.0", port)))
        .run(app)
        .await
}
