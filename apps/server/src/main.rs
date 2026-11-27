use std::path::Path;
use std::sync::Arc;

use tracing_subscriber::EnvFilter;

fn load_dotenv_if_present() {
    let path = Path::new(".env");
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((raw_key, raw_val)) = line.split_once('=') else {
            continue;
        };
        let key = raw_key.trim();
        if key.is_empty() || std::env::var_os(key).is_some() {
            continue;
        }
        let val = raw_val
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        std::env::set_var(key, val);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    load_dotenv_if_present();

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
    if let Ok(stored_modules) = vellum_server::persistence::module_repo::list_all(&db).await {
        for item in stored_modules {
            let _ = modules.register(item.module, item.source_kind, item.source_prompt);
        }
    }
    let llm_mode = std::env::var("VELLUM_LLM_MODE").unwrap_or_else(|_| "mock".into());
    let (provider, active_model): (Arc<dyn vellum_server::models::LlmProvider>, String) =
        if llm_mode == "live" {
            let live = vellum_server::models::live::LiveProvider::from_env()
                .map_err(std::io::Error::other)?;
            let model_name = live.model.clone();
            (Arc::new(live), model_name)
        } else {
            (
                Arc::new(vellum_server::models::MockProvider::default()),
                "mock".into(),
            )
        };

    let state = vellum_server::AppState::new(db, provider, modules, llm_mode.clone());
    let host = std::env::var("VELLUM_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("VELLUM_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3001);

    let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await?;
    tracing::info!(
        "vellum-server listening on http://{host}:{port} (llmMode={llm_mode}, model={active_model})"
    );
    axum::serve(listener, vellum_server::build_app(state)).await?;
    Ok(())
}
