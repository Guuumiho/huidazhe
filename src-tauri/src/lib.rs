use std::{fs, path::PathBuf, time::Instant};

use chrono::Utc;
use reqwest::Client;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const DEFAULT_MODEL: &str = "gpt-4.1-mini";
const SETTINGS_FILE_NAME: &str = "settings.json";
const DB_FILE_NAME: &str = "qa_records.db";

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct Settings {
    api_url: String,
    api_key: String,
    model: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistorySummary {
    id: i64,
    question_preview: String,
    created_at: i64,
    status: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryRecord {
    id: i64,
    question: String,
    answer: String,
    raw_response: Option<String>,
    created_at: i64,
    model: String,
    api_url: String,
    latency_ms: Option<i64>,
    status: String,
    error_message: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Debug, Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    content: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ResponsesRequest<'a> {
    model: &'a str,
    input: &'a str,
}

#[derive(Debug, Deserialize)]
struct ResponsesApiResponse {
    output_text: Option<String>,
    output: Option<Vec<ResponseOutputItem>>,
}

#[derive(Debug, Deserialize)]
struct ResponseOutputItem {
    content: Option<Vec<ResponseContentItem>>,
}

#[derive(Debug, Deserialize)]
struct ResponseContentItem {
    text: Option<String>,
}

#[derive(Debug)]
enum ApiKind {
    ChatCompletions,
    Responses,
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            list_history,
            list_history_records,
            get_history_item,
            ask
        ])
        .run(tauri::generate_context!())
        .expect("failed to run application");
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Result<Settings, String> {
    let path = settings_path(&app)?;
    if !path.exists() {
        return Ok(Settings::default());
    }

    let contents = fs::read_to_string(&path).map_err(|error| format!("Failed to read settings: {error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("Failed to parse settings: {error}"))
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    let config_dir = config_dir(&app)?;
    fs::create_dir_all(&config_dir).map_err(|error| format!("Failed to create config directory: {error}"))?;

    let sanitized = Settings {
        api_url: settings.api_url.trim().to_string(),
        api_key: settings.api_key.trim().to_string(),
        model: settings.model.trim().to_string(),
    };

    let contents = serde_json::to_string_pretty(&sanitized)
        .map_err(|error| format!("Failed to serialize settings: {error}"))?;
    fs::write(settings_path(&app)?, contents).map_err(|error| format!("Failed to save settings: {error}"))?;

    Ok(sanitized)
}

#[tauri::command]
fn list_history(app: AppHandle) -> Result<Vec<HistorySummary>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection
        .prepare(
            "SELECT id, question, created_at, status
             FROM qa_records
             ORDER BY created_at DESC, id DESC",
        )
        .map_err(|error| format!("Failed to read history: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            let question: String = row.get(1)?;
            Ok(HistorySummary {
                id: row.get(0)?,
                question_preview: summarize_question(&question),
                created_at: row.get(2)?,
                status: row.get(3)?,
            })
        })
        .map_err(|error| format!("Failed to read history: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read history: {error}"))
}

#[tauri::command]
fn get_history_item(app: AppHandle, id: i64) -> Result<HistoryRecord, String> {
    let connection = open_database(&app)?;
    connection
        .query_row(
            "SELECT id, question, answer, raw_response, created_at, model, api_url, latency_ms, status, error_message
             FROM qa_records
             WHERE id = ?1",
            [id],
            |row| {
                Ok(HistoryRecord {
                    id: row.get(0)?,
                    question: row.get(1)?,
                    answer: row.get(2)?,
                    raw_response: row.get(3)?,
                    created_at: row.get(4)?,
                    model: row.get(5)?,
                    api_url: row.get(6)?,
                    latency_ms: row.get(7)?,
                    status: row.get(8)?,
                    error_message: row.get(9)?,
                })
            },
        )
        .map_err(|error| format!("Failed to read history details: {error}"))
}

#[tauri::command]
fn list_history_records(app: AppHandle) -> Result<Vec<HistoryRecord>, String> {
    let connection = open_database(&app)?;
    let mut statement = connection
        .prepare(
            "SELECT id, question, answer, raw_response, created_at, model, api_url, latency_ms, status, error_message
             FROM qa_records
             ORDER BY created_at ASC, id ASC",
        )
        .map_err(|error| format!("Failed to read history records: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(HistoryRecord {
                id: row.get(0)?,
                question: row.get(1)?,
                answer: row.get(2)?,
                raw_response: row.get(3)?,
                created_at: row.get(4)?,
                model: row.get(5)?,
                api_url: row.get(6)?,
                latency_ms: row.get(7)?,
                status: row.get(8)?,
                error_message: row.get(9)?,
            })
        })
        .map_err(|error| format!("Failed to read history records: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read history records: {error}"))
}

#[tauri::command]
async fn ask(app: AppHandle, question: String) -> Result<HistoryRecord, String> {
    let trimmed_question = question.trim().to_string();
    if trimmed_question.is_empty() {
        return Err("Question cannot be empty.".to_string());
    }

    let settings = load_settings(app.clone())?;
    if settings.api_url.trim().is_empty() || settings.api_key.trim().is_empty() {
        return Err("Please fill in and save API URL and API Key first.".to_string());
    }

    let model = if settings.model.trim().is_empty() {
        DEFAULT_MODEL.to_string()
    } else {
        settings.model.trim().to_string()
    };

    let created_at = Utc::now().timestamp_millis();
    let (request_url, api_kind) = normalize_api_url(&settings.api_url);
    let client = Client::new();
    let timer = Instant::now();

    let response_result = match api_kind {
        ApiKind::ChatCompletions => {
            let payload = ChatCompletionRequest {
                model: &model,
                messages: vec![ChatMessage {
                    role: "user",
                    content: &trimmed_question,
                }],
            };

            client
                .post(&request_url)
                .bearer_auth(&settings.api_key)
                .json(&payload)
                .send()
                .await
        }
        ApiKind::Responses => {
            let payload = ResponsesRequest {
                model: &model,
                input: &trimmed_question,
            };

            client
                .post(&request_url)
                .bearer_auth(&settings.api_key)
                .json(&payload)
                .send()
                .await
        }
    };

    let response = match response_result {
        Ok(response) => response,
        Err(error) => {
            let message = format!("Request failed: {error}");
            insert_record(
                &app,
                &trimmed_question,
                "",
                None,
                created_at,
                &model,
                &request_url,
                Some(timer.elapsed().as_millis() as i64),
                "error",
                Some(&message),
            )?;
            return Err(message);
        }
    };

    let latency_ms = timer.elapsed().as_millis() as i64;
    let status_code = response.status();
    let raw_body = response
        .text()
        .await
        .map_err(|error| format!("Failed to read response body: {error}"))?;

    if !status_code.is_success() {
        let message = format!("API returned an error ({status_code}): {raw_body}");
        insert_record(
            &app,
            &trimmed_question,
            "",
            Some(&raw_body),
            created_at,
            &model,
            &request_url,
            Some(latency_ms),
            "error",
            Some(&message),
        )?;
        return Err(message);
    }

    let answer = match api_kind {
        ApiKind::ChatCompletions => parse_chat_completion_text(&raw_body),
        ApiKind::Responses => parse_responses_text(&raw_body),
    }
    .map_err(|error| format!("Failed to parse model response: {error}"))?;

    let record_id = insert_record(
        &app,
        &trimmed_question,
        &answer,
        Some(&raw_body),
        created_at,
        &model,
        &request_url,
        Some(latency_ms),
        "success",
        None,
    )?;

    Ok(HistoryRecord {
        id: record_id,
        question: trimmed_question,
        answer,
        raw_response: Some(raw_body),
        created_at,
        model,
        api_url: request_url,
        latency_ms: Some(latency_ms),
        status: "success".to_string(),
        error_message: None,
    })
}

fn normalize_api_url(input: &str) -> (String, ApiKind) {
    let trimmed = input.trim().trim_end_matches('/').to_string();
    if trimmed.ends_with("/v1") {
        return (format!("{trimmed}/chat/completions"), ApiKind::ChatCompletions);
    }
    if trimmed.ends_with("/responses") {
        return (trimmed, ApiKind::Responses);
    }
    if trimmed.ends_with("/chat/completions") {
        return (trimmed, ApiKind::ChatCompletions);
    }
    if trimmed.ends_with("/v1/responses") {
        return (trimmed, ApiKind::Responses);
    }
    if trimmed.ends_with("/v1/chat/completions") {
        return (trimmed, ApiKind::ChatCompletions);
    }
    if trimmed.contains("/v1/") {
        return (trimmed, ApiKind::ChatCompletions);
    }

    (format!("{trimmed}/v1/chat/completions"), ApiKind::ChatCompletions)
}

fn parse_chat_completion_text(raw_body: &str) -> Result<String, String> {
    let parsed: ChatCompletionResponse =
        serde_json::from_str(raw_body).map_err(|error| error.to_string())?;

    let first = parsed
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| "No choices returned by the API.".to_string())?;

    extract_content_value(first.message.content)
}

fn extract_content_value(value: serde_json::Value) -> Result<String, String> {
    match value {
        serde_json::Value::String(text) => Ok(text),
        serde_json::Value::Array(items) => {
            let mut buffer = Vec::new();
            for item in items {
                if let Some(text) = item.get("text").and_then(|inner| inner.as_str()) {
                    buffer.push(text.to_string());
                }
            }

            if buffer.is_empty() {
                Err("Could not extract text from the message content.".to_string())
            } else {
                Ok(buffer.join("\n"))
            }
        }
        _ => Err("Unsupported message content format.".to_string()),
    }
}

fn parse_responses_text(raw_body: &str) -> Result<String, String> {
    let parsed: ResponsesApiResponse =
        serde_json::from_str(raw_body).map_err(|error| error.to_string())?;

    if let Some(output_text) = parsed.output_text {
        if !output_text.trim().is_empty() {
            return Ok(output_text);
        }
    }

    let mut chunks = Vec::new();
    if let Some(output) = parsed.output {
        for item in output {
            if let Some(content) = item.content {
                for content_item in content {
                    if let Some(text) = content_item.text {
                        if !text.trim().is_empty() {
                            chunks.push(text);
                        }
                    }
                }
            }
        }
    }

    if chunks.is_empty() {
        Err("Could not extract text from the Responses API payload.".to_string())
    } else {
        Ok(chunks.join("\n"))
    }
}

fn summarize_question(question: &str) -> String {
    let single_line = question.replace('\n', " ").trim().to_string();
    let mut chars = single_line.chars();
    let preview: String = chars.by_ref().take(54).collect();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

fn insert_record(
    app: &AppHandle,
    question: &str,
    answer: &str,
    raw_response: Option<&str>,
    created_at: i64,
    model: &str,
    api_url: &str,
    latency_ms: Option<i64>,
    status: &str,
    error_message: Option<&str>,
) -> Result<i64, String> {
    let connection = open_database(app)?;
    connection
        .execute(
            "INSERT INTO qa_records (question, answer, raw_response, created_at, model, api_url, latency_ms, status, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                question,
                answer,
                raw_response,
                created_at,
                model,
                api_url,
                latency_ms,
                status,
                error_message
            ],
        )
        .map_err(|error| format!("Failed to write history: {error}"))?;

    Ok(connection.last_insert_rowid())
}

fn open_database(app: &AppHandle) -> Result<Connection, String> {
    let data_dir = data_dir(app)?;
    fs::create_dir_all(&data_dir).map_err(|error| format!("Failed to create data directory: {error}"))?;

    let connection =
        Connection::open(data_dir.join(DB_FILE_NAME)).map_err(|error| format!("Failed to open database: {error}"))?;

    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS qa_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                question TEXT NOT NULL,
                answer TEXT NOT NULL,
                raw_response TEXT,
                created_at INTEGER NOT NULL,
                model TEXT NOT NULL,
                api_url TEXT NOT NULL,
                latency_ms INTEGER,
                status TEXT NOT NULL,
                error_message TEXT
            );",
        )
        .map_err(|error| format!("Failed to initialize database: {error}"))?;

    connection
        .execute_batch("ALTER TABLE qa_records ADD COLUMN raw_response TEXT;")
        .ok();

    Ok(connection)
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join(SETTINGS_FILE_NAME))
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("Failed to locate config directory: {error}"))
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| format!("Failed to locate data directory: {error}"))
}
