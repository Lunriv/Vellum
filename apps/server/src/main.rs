use std::sync::Arc;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("vellum_server=info,tower_http=info")),
        )
        .init();

    let db_path = std::env::var("VELLUM_DB_PATH").unwrap_or_else(|_| "./data/vellum.sqlite".into());
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| format!("sqlite://{db_path}"));
    let db = vellum_server::persistence::db::Database::connect(&url).await?;
    vellum_server::runtime::recovery::scan_startup(&db).await?;

    let modules = Arc::new(vellum_server::modules::ModuleRegistry::load_default()?);
    let llm_mode = std::env::var("VELLUM_LLM_MODE").unwrap_or_else(|_| "mock".into());
    let provider: Arc<dyn vellum_server::models::LlmProvider> = if llm_mode == "live" {
        Arc::new(
            vellum_server::models::live::LiveProvider::from_env().map_err(std::io::Error::other)?,
        )
    } else {
        Arc::new(vellum_server::models::MockProvider::default())
    };

    let state = vellum_server::AppState::new(db, provider, modules, llm_mode);
    let host = std::env::var("VELLUM_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("VELLUM_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3001);

    let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await?;
    tracing::info!("vellum-server listening on http://{host}:{port}");
    axum::serve(listener, vellum_server::build_app(state)).await?;
    Ok(())
}
