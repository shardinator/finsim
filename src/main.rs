use poem::{
    get, handler,
    EndpointExt,
    http::{header::CONTENT_TYPE, StatusCode},
    listener::TcpListener,
    web::Data,
    Response, Route, Server,
};
use tera::{Context, Tera};

#[handler]
async fn home(tera: Data<&Tera>) -> poem::Result<Response> {
    let html = tera.render("home.html", &Context::new()).map_err(|e| {
        poem::Error::from_string(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    Ok(Response::builder()
        .header(CONTENT_TYPE, "text/html; charset=utf-8")
        .body(html))
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let tera = Tera::new("templates/**/*").expect("failed to load templates");
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);

    let app = Route::new().at("/", get(home)).data(tera);

    Server::new(TcpListener::bind(("0.0.0.0", port)))
        .run(app)
        .await
}
