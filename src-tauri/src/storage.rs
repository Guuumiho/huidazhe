use super::*;
use tauri::Manager;

pub(crate) fn open_database(app: &AppHandle) -> Result<Connection, String> {
    let data_dir = data_dir(app)?;
    fs::create_dir_all(&data_dir).map_err(|error| format!("Failed to create data directory: {error}"))?;

    let connection =
        Connection::open(data_dir.join(DB_FILE_NAME)).map_err(|error| format!("Failed to open database: {error}"))?;

    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS qa_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id INTEGER,
                question TEXT NOT NULL,
                answer TEXT NOT NULL,
                raw_response TEXT,
                fallback_notice TEXT,
                created_at INTEGER NOT NULL,
                model TEXT NOT NULL,
                api_url TEXT NOT NULL,
                prompt_mode TEXT NOT NULL DEFAULT 'single',
                latency_ms INTEGER,
                status TEXT NOT NULL,
                error_message TEXT
            );

            CREATE TABLE IF NOT EXISTS knowledge_nodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                normalized_title TEXT NOT NULL UNIQUE,
                summary TEXT NOT NULL,
                aliases_json TEXT NOT NULL DEFAULT '[]',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS knowledge_edges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_node_id INTEGER NOT NULL,
                to_node_id INTEGER NOT NULL,
                relation_type TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                UNIQUE(from_node_id, to_node_id, relation_type)
            );

            CREATE TABLE IF NOT EXISTS knowledge_sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                node_id INTEGER NOT NULL,
                qa_record_id INTEGER NOT NULL UNIQUE,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS knowledge_task_state (
                task_name TEXT PRIMARY KEY,
                last_run_at INTEGER,
                last_status TEXT NOT NULL,
                last_error TEXT,
                last_processed_qa_id INTEGER
            );

            CREATE TABLE IF NOT EXISTS conversations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                mode TEXT NOT NULL DEFAULT 'single',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS conversation_session_memory (
                conversation_id INTEGER PRIMARY KEY,
                memory_json TEXT NOT NULL DEFAULT '{}',
                updated_at INTEGER NOT NULL
            );",
        )
        .map_err(|error| format!("Failed to initialize database: {error}"))?;

    connection
        .execute_batch("ALTER TABLE qa_records ADD COLUMN raw_response TEXT;")
        .ok();
    connection
        .execute_batch("ALTER TABLE qa_records ADD COLUMN conversation_id INTEGER;")
        .ok();
    connection
        .execute_batch("ALTER TABLE qa_records ADD COLUMN fallback_notice TEXT;")
        .ok();
    connection
        .execute_batch("ALTER TABLE qa_records ADD COLUMN prompt_mode TEXT NOT NULL DEFAULT 'single';")
        .ok();
    connection
        .execute("UPDATE qa_records SET prompt_mode = 'single' WHERE prompt_mode IS NULL OR prompt_mode = ''", [])
        .ok();

    backfill_default_conversation(&connection)?;

    Ok(connection)
}

fn backfill_default_conversation(connection: &Connection) -> Result<(), String> {
    let missing_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM qa_records WHERE conversation_id IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Failed to inspect conversation migration: {error}"))?;

    if missing_count == 0 {
        return Ok(());
    }

    let now = Utc::now().timestamp_millis();
    connection
        .execute(
            "INSERT INTO conversations (title, mode, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params!["已导入历史", "single", now, now],
        )
        .map_err(|error| format!("Failed to create migrated conversation: {error}"))?;

    let conversation_id = connection.last_insert_rowid();
    connection
        .execute(
            "UPDATE qa_records
             SET conversation_id = ?1
             WHERE conversation_id IS NULL",
            [conversation_id],
        )
        .map_err(|error| format!("Failed to backfill conversation ids: {error}"))?;

    Ok(())
}

pub(crate) fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join(SETTINGS_FILE_NAME))
}

pub(crate) fn model_call_log_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join(MODEL_CALL_LOG_FILE_NAME))
}

pub(crate) fn append_model_call_log(app: &AppHandle, entry: &ModelCallLogEntry) -> Result<(), String> {
    let path = model_call_log_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Failed to create log directory: {error}"))?;
    }

    let line = serde_json::to_string(entry).map_err(|error| format!("Failed to serialize model log entry: {error}"))?;
    let mut content = line;
    content.push('\n');

    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("Failed to open model log file: {error}"))?;
    file.write_all(content.as_bytes())
        .map_err(|error| format!("Failed to write model log file: {error}"))?;
    Ok(())
}

pub(crate) fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("Failed to locate config directory: {error}"))
}

pub(crate) fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| format!("Failed to locate data directory: {error}"))
}
