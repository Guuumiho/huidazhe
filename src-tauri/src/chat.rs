use super::*;
use std::time::Instant;

#[tauri::command]
pub(crate) fn list_conversations(app: AppHandle) -> Result<Vec<ConversationSummary>, String> {
    let connection = crate::storage::open_database(&app)?;
    ensure_initial_conversation(&connection)?;

    let mut statement = connection
        .prepare(
            "SELECT id, title, mode, created_at, updated_at
             FROM conversations
             ORDER BY updated_at DESC, id DESC",
        )
        .map_err(|error| format!("Failed to read conversations: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(ConversationSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                mode: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|error| format!("Failed to read conversations: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read conversations: {error}"))
}

#[tauri::command]
pub(crate) fn create_conversation(app: AppHandle, mode: Option<String>) -> Result<ConversationSummary, String> {
    let connection = crate::storage::open_database(&app)?;
    let now = Utc::now().timestamp_millis();
    let title = format!("新对话 {}", chrono::Local::now().format("%m/%d %H:%M"));
    let normalized_mode = if mode.as_deref() == Some("memory") {
        "memory"
    } else {
        "single"
    };

    connection
        .execute(
            "INSERT INTO conversations (title, mode, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![title, normalized_mode, now, now],
        )
        .map_err(|error| format!("Failed to create conversation: {error}"))?;

    let id = connection.last_insert_rowid();
    let empty_memory =
        serde_json::to_string(&SessionMemory::default()).map_err(|error| format!("Failed to initialize session memory: {error}"))?;
    connection
        .execute(
            "INSERT OR IGNORE INTO conversation_session_memory (conversation_id, memory_json, updated_at)
             VALUES (?1, ?2, ?3)",
            params![id, empty_memory, now],
        )
        .map_err(|error| format!("Failed to initialize session memory: {error}"))?;

    Ok(ConversationSummary {
        id,
        title: connection
            .query_row("SELECT title FROM conversations WHERE id = ?1", [id], |row| row.get(0))
            .map_err(|error| format!("Failed to read new conversation: {error}"))?,
        mode: normalized_mode.to_string(),
        created_at: now,
        updated_at: now,
    })
}

#[tauri::command]
pub(crate) fn delete_conversation(app: AppHandle, conversation_id: i64) -> Result<Vec<ConversationSummary>, String> {
    let connection = crate::storage::open_database(&app)?;
    ensure_initial_conversation(&connection)?;

    let total_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
        .map_err(|error| format!("Failed to count conversations: {error}"))?;

    if total_count <= 1 {
        return Err("At least one conversation must remain.".to_string());
    }

    connection
        .execute("DELETE FROM qa_records WHERE conversation_id = ?1", [conversation_id])
        .map_err(|error| format!("Failed to delete conversation history: {error}"))?;

    connection
        .execute("DELETE FROM conversations WHERE id = ?1", [conversation_id])
        .map_err(|error| format!("Failed to delete conversation: {error}"))?;

    connection
        .execute(
            "DELETE FROM conversation_session_memory WHERE conversation_id = ?1",
            [conversation_id],
        )
        .map_err(|error| format!("Failed to delete conversation memory: {error}"))?;

    let mut statement = connection
        .prepare(
            "SELECT id, title, mode, created_at, updated_at
             FROM conversations
             ORDER BY updated_at DESC, id DESC",
        )
        .map_err(|error| format!("Failed to reload conversations: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(ConversationSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                mode: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|error| format!("Failed to reload conversations: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to reload conversations: {error}"))
}

#[tauri::command]
pub(crate) fn update_conversation_mode(
    app: AppHandle,
    conversation_id: i64,
    mode: String,
) -> Result<ConversationSummary, String> {
    let connection = crate::storage::open_database(&app)?;
    let normalized_mode = if mode == "memory" { "memory" } else { "single" };
    let updated_at = Utc::now().timestamp_millis();

    connection
        .execute(
            "UPDATE conversations
             SET mode = ?1, updated_at = ?2
             WHERE id = ?3",
            params![normalized_mode, updated_at, conversation_id],
        )
        .map_err(|error| format!("Failed to update conversation mode: {error}"))?;

    load_conversation_summary(&connection, conversation_id)
}

#[tauri::command]
pub(crate) fn list_history(app: AppHandle) -> Result<Vec<HistorySummary>, String> {
    let connection = crate::storage::open_database(&app)?;
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
pub(crate) fn get_history_item(app: AppHandle, id: i64) -> Result<HistoryRecord, String> {
    let connection = crate::storage::open_database(&app)?;
    connection
        .query_row(
            "SELECT id, conversation_id, question, answer, raw_response, fallback_notice, created_at, model, api_url, latency_ms, status, error_message
             FROM qa_records
             WHERE id = ?1",
            [id],
            |row| map_history_record(row),
        )
        .map_err(|error| format!("Failed to read history details: {error}"))
}

#[tauri::command]
pub(crate) fn list_history_records(app: AppHandle, conversation_id: i64) -> Result<Vec<HistoryRecord>, String> {
    let connection = crate::storage::open_database(&app)?;
    let mut statement = connection
        .prepare(
            "SELECT id, conversation_id, question, answer, raw_response, fallback_notice, created_at, model, api_url, latency_ms, status, error_message
             FROM qa_records
             WHERE conversation_id = ?1
             ORDER BY created_at ASC, id ASC",
        )
        .map_err(|error| format!("Failed to read history records: {error}"))?;

    let rows = statement
        .query_map([conversation_id], map_history_record)
        .map_err(|error| format!("Failed to read history records: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read history records: {error}"))
}

#[tauri::command]
pub(crate) async fn ask(
    app: AppHandle,
    conversation_id: i64,
    question: String,
    use_short_term_memory: Option<bool>,
) -> Result<AskResponse, String> {
    let trimmed_question = question.trim().to_string();
    if trimmed_question.is_empty() {
        return Err("Question cannot be empty.".to_string());
    }

    let settings = crate::settings::load_settings(app.clone())?;
    if settings.api_url.trim().is_empty() || settings.api_key.trim().is_empty() {
        return Err("Please fill in and save API URL and API Key first.".to_string());
    }

    let model = ASK_MODEL.to_string();
    let created_at = Utc::now().timestamp_millis();
    let use_memory = use_short_term_memory.unwrap_or(false);
    let prompt_mode = if use_memory { "memory" } else { "single" };
    let session_memory = if use_memory {
        load_session_memory(&app, conversation_id)?
    } else {
        SessionMemory::default()
    };
    let system_prompt = build_chat_system_prompt();
    let user_prompt = build_chat_user_prompt(
        &trimmed_question,
        if use_memory { Some(&session_memory) } else { None },
    );
    let short_term_memory = if use_memory {
        fetch_short_term_memory(&app, conversation_id, SHORT_TERM_MEMORY_ROUNDS)?
    } else {
        Vec::new()
    };

    let timer = Instant::now();
    let (normalized_url, _) = normalize_api_url(&settings.api_url);
    let primary_attempt = execute_chat_attempt(
        &app,
        &settings,
        &model,
        &system_prompt,
        &user_prompt,
        &short_term_memory,
    )
    .await;

    let (final_model, raw_body, answer, fallback_notice) = match primary_attempt {
        Ok(result) => (model.clone(), result.raw_body, result.answer, None),
        Err(_) => {
            match execute_chat_attempt(
                &app,
                &settings,
                &model,
                &system_prompt,
                &user_prompt,
                &short_term_memory,
            )
            .await
            {
                Ok(result) => (model.clone(), result.raw_body, result.answer, None),
                Err(_) => match execute_chat_attempt(
                    &app,
                    &settings,
                    AUXILIARY_MODEL,
                    &system_prompt,
                    &user_prompt,
                    &short_term_memory,
                )
                .await
                {
                    Ok(result) => (
                        AUXILIARY_MODEL.to_string(),
                        result.raw_body,
                        result.answer,
                        Some("gpt-5.4请求失败，此问题切换成gpt-5.4-mini".to_string()),
                    ),
                    Err(_) => {
                        return Ok(AskResponse {
                            ok: false,
                            record: None,
                            failure_message: Some("大模型api暂不可用，稍后重试".to_string()),
                            retry_available: true,
                        });
                    }
                },
            }
        }
    };

    let latency_ms = timer.elapsed().as_millis() as i64;

    let record_id = insert_record(
        &app,
        conversation_id,
        &trimmed_question,
        &answer,
        Some(&raw_body),
        fallback_notice.as_deref(),
        created_at,
        &final_model,
        &normalized_url,
        prompt_mode,
        Some(latency_ms),
        "success",
        None,
    )?;

    update_conversation_after_message(
        &crate::storage::open_database(&app)?,
        conversation_id,
        created_at,
    )?;

    let _ = refresh_conversation_title(&app, &settings, conversation_id, &trimmed_question, &answer).await;

    if use_memory {
        let _ = refresh_session_memory(&app, &settings, conversation_id, &trimmed_question, &answer).await;
    }

    Ok(AskResponse {
        ok: true,
        record: Some(HistoryRecord {
            id: record_id,
            conversation_id,
            question: trimmed_question,
            answer,
            raw_response: Some(raw_body),
            fallback_notice,
            created_at,
            model: final_model,
            api_url: normalized_url,
            latency_ms: Some(latency_ms),
            status: "success".to_string(),
            error_message: None,
        }),
        failure_message: None,
        retry_available: false,
    })
}

pub(crate) async fn send_model_text_request(
    app: &AppHandle,
    settings: &Settings,
    model: &str,
    purpose: &str,
    system_prompt: Option<&str>,
    user_prompt: &str,
    short_term_memory: &[MemoryMessage],
) -> Result<String, String> {
    let (request_url, api_kind) = normalize_api_url(&settings.api_url);
    let client = Client::new();

    let (request_body, response) = match api_kind {
        ApiKind::ChatCompletions => {
            let mut messages = Vec::new();
            if let Some(system_prompt) = system_prompt {
                messages.push(ChatMessage {
                    role: "system",
                    content: system_prompt,
                });
            }
            for memory_message in short_term_memory {
                messages.push(ChatMessage {
                    role: &memory_message.role,
                    content: &memory_message.content,
                });
            }
            messages.push(ChatMessage {
                role: "user",
                content: user_prompt,
            });

            let payload = ChatCompletionRequest { model, messages };
            let request_body =
                serde_json::to_value(&payload).map_err(|error| format!("Failed to serialize request body: {error}"))?;
            let response = client
                .post(&request_url)
                .bearer_auth(&settings.api_key)
                .json(&payload)
                .send()
                .await
                .map_err(|error| {
                    let error_message = error.to_string();
                    let _ = crate::storage::append_model_call_log(
                        app,
                        &ModelCallLogEntry {
                            timestamp: Utc::now().timestamp_millis(),
                            purpose: purpose.to_string(),
                            model: model.to_string(),
                            api_url: request_url.clone(),
                            api_kind: "chat_completions".to_string(),
                            request_body: request_body.clone(),
                            response_status: None,
                            response_ok: false,
                            response_body: None,
                            error: Some(error_message.clone()),
                        },
                    );
                    error_message
                })?;
            (request_body, response)
        }
        ApiKind::Responses => {
            let combined_prompt = build_responses_input(system_prompt, short_term_memory, user_prompt);
            let payload = ResponsesRequest {
                model,
                input: &combined_prompt,
            };
            let request_body =
                serde_json::to_value(&payload).map_err(|error| format!("Failed to serialize request body: {error}"))?;
            let response = client
                .post(&request_url)
                .bearer_auth(&settings.api_key)
                .json(&payload)
                .send()
                .await
                .map_err(|error| {
                    let error_message = error.to_string();
                    let _ = crate::storage::append_model_call_log(
                        app,
                        &ModelCallLogEntry {
                            timestamp: Utc::now().timestamp_millis(),
                            purpose: purpose.to_string(),
                            model: model.to_string(),
                            api_url: request_url.clone(),
                            api_kind: "responses".to_string(),
                            request_body: request_body.clone(),
                            response_status: None,
                            response_ok: false,
                            response_body: None,
                            error: Some(error_message.clone()),
                        },
                    );
                    error_message
                })?;
            (request_body, response)
        }
    };

    let status = response.status();
    let raw_body = response
        .text()
        .await
        .map_err(|error| format!("Failed to read response body: {error}"))?;

    let api_kind_label = match api_kind {
        ApiKind::ChatCompletions => "chat_completions",
        ApiKind::Responses => "responses",
    };
    let status_u16 = status.as_u16();
    let status_ok = status.is_success();

    let _ = crate::storage::append_model_call_log(
        app,
        &ModelCallLogEntry {
            timestamp: Utc::now().timestamp_millis(),
            purpose: purpose.to_string(),
            model: model.to_string(),
            api_url: request_url.clone(),
            api_kind: api_kind_label.to_string(),
            request_body,
            response_status: Some(status_u16),
            response_ok: status_ok,
            response_body: Some(raw_body.clone()),
            error: if status_ok {
                None
            } else {
                Some(format!("API returned an error ({status}): {raw_body}"))
            },
        },
    );

    if !status_ok {
        return Err(format!("API returned an error ({status}): {raw_body}"));
    }

    Ok(raw_body)
}

struct ChatAttemptResult {
    raw_body: String,
    answer: String,
}

async fn execute_chat_attempt(
    app: &AppHandle,
    settings: &Settings,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    short_term_memory: &[MemoryMessage],
) -> Result<ChatAttemptResult, String> {
    let raw_body = send_model_text_request(
        app,
        settings,
        model,
        "chat_answer",
        Some(system_prompt),
        user_prompt,
        short_term_memory,
    )
    .await?;

    let answer = parse_model_text(&settings.api_url, &raw_body)
        .map_err(|error| format!("Failed to parse model response: {error}"))?;

    Ok(ChatAttemptResult { raw_body, answer })
}

fn build_responses_input(
    system_prompt: Option<&str>,
    short_term_memory: &[MemoryMessage],
    user_prompt: &str,
) -> String {
    let mut sections = Vec::new();

    if let Some(system_prompt) = system_prompt {
        sections.push(format!("System:\n{system_prompt}"));
    }

    if !short_term_memory.is_empty() {
        let memory_text = short_term_memory
            .iter()
            .map(|message| match message.role.as_str() {
                "assistant" => format!("Assistant:\n{}", message.content),
                _ => format!("User:\n{}", message.content),
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        sections.push(format!("Recent conversation context:\n{memory_text}"));
    }

    sections.push(format!("User:\n{user_prompt}"));
    sections.join("\n\n")
}

fn build_chat_system_prompt() -> String {
    ASK_SYSTEM_PROMPT.to_string()
}

fn build_chat_user_prompt(question: &str, session_memory: Option<&SessionMemory>) -> String {
    if let Some(session_memory) = session_memory {
        let session_memory_json =
            serde_json::to_string(session_memory).unwrap_or_else(|_| "{}".to_string());
        format!(
            "[Session Memory]\n{}\n\n[Current Question]\n{}",
            session_memory_json, question
        )
    } else {
        question.to_string()
    }
}

async fn refresh_session_memory(
    app: &AppHandle,
    settings: &Settings,
    conversation_id: i64,
    user_question: &str,
    assistant_answer: &str,
) -> Result<(), String> {
    let old_memory = load_session_memory(app, conversation_id)?;
    let recent_dialogue = fetch_recent_dialogue_for_memory_update(
        app,
        conversation_id,
        SESSION_MEMORY_RECENT_ROUNDS,
    )?;
    let old_memory_json =
        serde_json::to_string(&old_memory).map_err(|error| format!("Failed to serialize session memory: {error}"))?;
    let latest_dialogue = format_recent_dialogue(&recent_dialogue, user_question, assistant_answer);

    let system_prompt = "你是一个对话记忆压缩器。你的任务不是复述聊天内容，而是维护“会话状态”。\n\
请根据“旧的会话记忆”和“最新几轮对话”，输出更新后的会话记忆。\n\
要求：\n\
1. 只保留对后续对话有用的信息。\n\
2. 删除寒暄、重复、无关表述。\n\
3. 区分“已确认事实”和“推测”。\n\
4. 明确当前目标、当前进度、待确认问题、下一步动作。\n\
5. 输出必须是 JSON。\n\
6. 尽量简洁，但不能遗漏关键约束和关键决策。\n\
7. 如果新信息与旧信息冲突，以最新且明确确认的信息为准。";

    let user_prompt = format!(
        "旧的会话记忆：\n{old_memory_json}\n\n最新对话：\n{latest_dialogue}\n\n请输出更新后的会话记忆 JSON，格式如下：\n{{\n  \"session_goal\": \"\",\n  \"confirmed_facts\": [],\n  \"constraints\": [],\n  \"preferences\": [],\n  \"progress\": [],\n  \"open_questions\": [],\n  \"next_action\": \"\",\n  \"key_decisions\": [],\n  \"risks_or_issues\": []\n}}"
    );

    let raw_text = send_model_text_request(
        app,
        settings,
        AUXILIARY_MODEL,
        "session_memory_update",
        Some(system_prompt),
        &user_prompt,
        &[],
    )
    .await?;

    let parsed_text = parse_model_text(&settings.api_url, &raw_text)
        .map_err(|error| format!("Failed to parse session memory model response: {error}"))?;
    let json_text = extract_json_object(&parsed_text)
        .ok_or_else(|| format!("Session memory update did not return valid JSON: {}", sanitize_text(&parsed_text, 280)))?;
    let next_memory: SessionMemory = serde_json::from_str(&json_text)
        .map_err(|error| format!("Failed to parse session memory JSON: {error}"))?;
    save_session_memory(app, conversation_id, &next_memory)?;
    Ok(())
}

async fn refresh_conversation_title(
    app: &AppHandle,
    settings: &Settings,
    conversation_id: i64,
    question: &str,
    answer: &str,
) -> Result<(), String> {
    let connection = crate::storage::open_database(app)?;
    let current_title: String = connection
        .query_row(
            "SELECT title FROM conversations WHERE id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("Failed to read conversation title: {error}"))?;

    if !current_title.starts_with("新对话") {
        return Ok(());
    }

    let system_prompt = "你是一个对话主题压缩器。请根据用户的第一条问题和对应回答，总结一个 10 字以内的主题。\
只输出主题本身，不要解释，不要标点，不要换行。";
    let user_prompt = format!("用户问题：\n{question}\n\n助手回答：\n{answer}");
    let raw_text = send_model_text_request(
        app,
        settings,
        AUXILIARY_MODEL,
        "conversation_title",
        Some(system_prompt),
        &user_prompt,
        &[],
    )
    .await?;

    let title_text = parse_model_text(&settings.api_url, &raw_text)
        .map_err(|error| format!("Failed to parse conversation title response: {error}"))?;
    let next_title = sanitize_generated_title(&title_text);
    if next_title.is_empty() {
        return Ok(());
    }

    connection
        .execute(
            "UPDATE conversations
             SET title = ?1
             WHERE id = ?2",
            params![next_title, conversation_id],
        )
        .map_err(|error| format!("Failed to update conversation title: {error}"))?;

    Ok(())
}

fn parse_model_text(api_url: &str, raw_body: &str) -> Result<String, String> {
    match normalize_api_url(api_url).1 {
        ApiKind::ChatCompletions => parse_chat_completion_text(raw_body),
        ApiKind::Responses => parse_responses_text(raw_body),
    }
}

pub(crate) fn normalize_api_url(input: &str) -> (String, ApiKind) {
    let trimmed = input.trim().trim_end_matches('/').to_string();
    if trimmed.ends_with("/responses") || trimmed.ends_with("/v1/responses") {
        return (trimmed, ApiKind::Responses);
    }
    if trimmed.ends_with("/chat/completions") || trimmed.ends_with("/v1/chat/completions") {
        return (trimmed, ApiKind::ChatCompletions);
    }
    if trimmed.ends_with("/v1") {
        return (format!("{trimmed}/chat/completions"), ApiKind::ChatCompletions);
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
    let parsed: ResponsesApiResponse = serde_json::from_str(raw_body).map_err(|error| error.to_string())?;

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

fn sanitize_generated_title(text: &str) -> String {
    let cleaned = text
        .replace(['\r', '\n'], " ")
        .replace(['【', '】', '[', ']'], "")
        .replace("主题：", "")
        .replace("主题", "")
        .trim()
        .to_string();

    cleaned.chars().take(10).collect::<String>().trim().to_string()
}

pub(crate) fn sanitize_text(text: &str, max_chars: usize) -> String {
    let sanitized = text.replace('\n', " ").trim().to_string();
    let mut chars = sanitized.chars();
    let preview: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

pub(crate) fn summarize_question(question: &str) -> String {
    sanitize_text(question, 54)
}

fn fetch_short_term_memory(app: &AppHandle, conversation_id: i64, rounds: usize) -> Result<Vec<MemoryMessage>, String> {
    let connection = crate::storage::open_database(app)?;
    let mut statement = connection
        .prepare(
            "SELECT question, answer
             FROM qa_records
             WHERE status = 'success'
                AND answer <> ''
                AND conversation_id = ?1
                AND prompt_mode = 'memory'
             ORDER BY created_at DESC, id DESC
             LIMIT ?2",
        )
        .map_err(|error| format!("Failed to read short-term memory: {error}"))?;

    let rows = statement
        .query_map(params![conversation_id, rounds as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("Failed to read short-term memory: {error}"))?;

    let mut pairs = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read short-term memory: {error}"))?;
    pairs.reverse();

    let mut messages = Vec::new();
    for (question, answer) in pairs {
        messages.push(MemoryMessage {
            role: "user".to_string(),
            content: question,
        });
        messages.push(MemoryMessage {
            role: "assistant".to_string(),
            content: answer,
        });
    }

    Ok(messages)
}

fn load_session_memory(app: &AppHandle, conversation_id: i64) -> Result<SessionMemory, String> {
    let connection = crate::storage::open_database(app)?;
    let raw_json: Option<String> = connection
        .query_row(
            "SELECT memory_json FROM conversation_session_memory WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("Failed to read session memory: {error}"))?;

    match raw_json {
        Some(raw_json) => serde_json::from_str(&raw_json)
            .map_err(|error| format!("Failed to parse session memory: {error}")),
        None => Ok(SessionMemory::default()),
    }
}

fn save_session_memory(
    app: &AppHandle,
    conversation_id: i64,
    session_memory: &SessionMemory,
) -> Result<(), String> {
    let connection = crate::storage::open_database(app)?;
    let memory_json = serde_json::to_string(session_memory)
        .map_err(|error| format!("Failed to serialize session memory: {error}"))?;
    let updated_at = Utc::now().timestamp_millis();

    connection
        .execute(
            "INSERT INTO conversation_session_memory (conversation_id, memory_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(conversation_id) DO UPDATE SET
               memory_json = excluded.memory_json,
               updated_at = excluded.updated_at",
            params![conversation_id, memory_json, updated_at],
        )
        .map_err(|error| format!("Failed to write session memory: {error}"))?;

    Ok(())
}

fn fetch_recent_dialogue_for_memory_update(
    app: &AppHandle,
    conversation_id: i64,
    rounds: usize,
) -> Result<Vec<(String, String)>, String> {
    let connection = crate::storage::open_database(app)?;
    let mut statement = connection
        .prepare(
            "SELECT question, answer
             FROM qa_records
             WHERE status = 'success'
               AND answer <> ''
               AND conversation_id = ?1
             ORDER BY created_at DESC, id DESC
             LIMIT ?2",
        )
        .map_err(|error| format!("Failed to read recent dialogue: {error}"))?;

    let rows = statement
        .query_map(params![conversation_id, rounds as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("Failed to read recent dialogue: {error}"))?;

    let mut pairs = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read recent dialogue: {error}"))?;
    pairs.reverse();
    Ok(pairs)
}

fn format_recent_dialogue(
    pairs: &[(String, String)],
    current_question: &str,
    current_answer: &str,
) -> String {
    let mut lines = Vec::new();

    for (question, answer) in pairs {
        lines.push(format!(
            "用户: {}\n助手: {}",
            sanitize_text(question, SESSION_MEMORY_MAX_TEXT_CHARS),
            sanitize_text(answer, SESSION_MEMORY_MAX_TEXT_CHARS)
        ));
    }

    if pairs.is_empty()
        || pairs
            .last()
            .map(|(question, answer)| question != current_question || answer != current_answer)
            .unwrap_or(true)
    {
        lines.push(format!(
            "用户: {}\n助手: {}",
            sanitize_text(current_question, SESSION_MEMORY_MAX_TEXT_CHARS),
            sanitize_text(current_answer, SESSION_MEMORY_MAX_TEXT_CHARS)
        ));
    }

    lines.join("\n")
}

fn extract_json_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(text[start..=end].to_string())
}

fn insert_record(
    app: &AppHandle,
    conversation_id: i64,
    question: &str,
    answer: &str,
    raw_response: Option<&str>,
    fallback_notice: Option<&str>,
    created_at: i64,
    model: &str,
    api_url: &str,
    prompt_mode: &str,
    latency_ms: Option<i64>,
    status: &str,
    error_message: Option<&str>,
) -> Result<i64, String> {
    let connection = crate::storage::open_database(app)?;
    connection
        .execute(
            "INSERT INTO qa_records (conversation_id, question, answer, raw_response, fallback_notice, created_at, model, api_url, prompt_mode, latency_ms, status, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                conversation_id,
                question,
                answer,
                raw_response,
                fallback_notice,
                created_at,
                model,
                api_url,
                prompt_mode,
                latency_ms,
                status,
                error_message
            ],
        )
        .map_err(|error| format!("Failed to write history: {error}"))?;

    Ok(connection.last_insert_rowid())
}

fn map_history_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryRecord> {
    Ok(HistoryRecord {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        question: row.get(2)?,
        answer: row.get(3)?,
        raw_response: row.get(4)?,
        fallback_notice: row.get(5)?,
        created_at: row.get(6)?,
        model: row.get(7)?,
        api_url: row.get(8)?,
        latency_ms: row.get(9)?,
        status: row.get(10)?,
        error_message: row.get(11)?,
    })
}

fn ensure_initial_conversation(connection: &Connection) -> Result<(), String> {
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
        .map_err(|error| format!("Failed to inspect conversations: {error}"))?;

    if count > 0 {
        return Ok(());
    }

    let now = Utc::now().timestamp_millis();
    connection
        .execute(
            "INSERT INTO conversations (title, mode, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params!["新对话", "single", now, now],
        )
        .map_err(|error| format!("Failed to create initial conversation: {error}"))?;

    Ok(())
}

fn load_conversation_summary(connection: &Connection, conversation_id: i64) -> Result<ConversationSummary, String> {
    connection
        .query_row(
            "SELECT id, title, mode, created_at, updated_at
             FROM conversations
             WHERE id = ?1",
            [conversation_id],
            |row| {
                Ok(ConversationSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    mode: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .map_err(|error| format!("Failed to load conversation: {error}"))
}

fn update_conversation_after_message(
    connection: &Connection,
    conversation_id: i64,
    updated_at: i64,
) -> Result<(), String> {
    connection
        .execute(
            "UPDATE conversations
             SET updated_at = ?1
             WHERE id = ?2",
            params![updated_at, conversation_id],
        )
        .map_err(|error| format!("Failed to update conversation metadata: {error}"))?;

    Ok(())
}
