use chrono::Utc;

use super::db::Database;
use crate::modules::ModuleIr;

pub struct StoredCustomModule {
    pub module: ModuleIr,
    pub source_kind: String,
    pub source_prompt: Option<String>,
}

pub async fn upsert(
    db: &Database,
    module: &ModuleIr,
    source_kind: &str,
    source_prompt: Option<&str>,
) -> Result<(), sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    let ir_json = serde_json::to_string(module).unwrap_or_else(|_| "{}".into());
    sqlx::query(
        "INSERT INTO custom_modules(module_id,version,title,source_kind,source_prompt,ir_json,created_at,updated_at)
         VALUES(?,?,?,?,?,?,?,?)
         ON CONFLICT(module_id) DO UPDATE SET
           version=excluded.version,
           title=excluded.title,
           source_kind=excluded.source_kind,
           source_prompt=excluded.source_prompt,
           ir_json=excluded.ir_json,
           updated_at=excluded.updated_at",
    )
    .bind(&module.module_id)
    .bind(&module.version)
    .bind(&module.title)
    .bind(source_kind)
    .bind(source_prompt)
    .bind(ir_json)
    .bind(&now)
    .bind(&now)
    .execute(&db.pool)
    .await?;
    Ok(())
}

pub async fn list_all(db: &Database) -> Result<Vec<StoredCustomModule>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, Option<String>, String)>(
        "SELECT source_kind,source_prompt,ir_json FROM custom_modules ORDER BY updated_at DESC",
    )
    .fetch_all(&db.pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|(source_kind, source_prompt, ir_json)| {
            serde_json::from_str::<ModuleIr>(&ir_json)
                .ok()
                .map(|module| StoredCustomModule {
                    module,
                    source_kind,
                    source_prompt,
                })
        })
        .collect())
}

pub async fn delete(db: &Database, module_id: &str) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("DELETE FROM custom_modules WHERE module_id=?")
        .bind(module_id)
        .execute(&db.pool)
        .await?;
    Ok(res.rows_affected() > 0)
}
