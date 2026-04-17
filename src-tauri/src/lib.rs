use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
};

use chrono::Utc;
use reqwest::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

mod chat;
mod knowledge;
mod settings;
mod storage;

const ASK_MODEL: &str = "gpt-5.4";
const AUXILIARY_MODEL: &str = "gpt-5.4-mini";
const KNOWLEDGE_MODEL: &str = AUXILIARY_MODEL;
const DEFAULT_THEME: &str = "default-theme";
const SETTINGS_FILE_NAME: &str = "settings.json";
const DB_FILE_NAME: &str = "qa_records.db";
const MODEL_CALL_LOG_FILE_NAME: &str = "model_calls.jsonl";
const KNOWLEDGE_TASK_NAME: &str = "knowledge_map";
const KNOWLEDGE_CHECK_INTERVAL_MS: i64 = 60 * 60 * 1000;
const KNOWLEDGE_BATCH_LIMIT: usize = 40;
const SHORT_TERM_MEMORY_ROUNDS: usize = 6;
const SESSION_MEMORY_RECENT_ROUNDS: usize = 3;
const SESSION_MEMORY_MAX_TEXT_CHARS: usize = 1200;
const ASK_SYSTEM_PROMPT: &str = "你是一个高密度、低废话的助手。\n\n请严格遵守以下规则：\n\n1. 先判断用户问题属于：\n- 简单问题：可直接回答\n- 复杂问题：涉及分析、方案、比较、规划\n- 模糊问题：信息不足或目标不清\n\n2. 简单问题：\n- 直接给结论\n- 用最少字数\n- 不做背景解释\n- 不主动延伸\n\n3. 复杂问题：\n- 先给框架，再给每个模块的核心点\n- 框架控制在 3~6 个模块\n- 每个模块只写最关键内容\n- 不一次性展开全部细节\n- 结尾给一个明确下一步\n\n4. 模糊问题：\n- 先问 1~3 个关键澄清问题\n- 或先把问题拆成几个方向让用户选\n- 不在信息不足时长篇作答\n\n5. 表达要求：\n- 短句\n- 列表优先\n- 禁止空话、套话、重复\n- 如果一句话能说清，就不要写一段\n- 回答宁可简短，也不要冗长\n\n6. 默认目标：\n- 帮助用户收敛问题\n- 推动用户逐步提供更具体的信息\n- 只回答当前层级，不抢答后续层级\n\n输出格式规则：\n- 简单问题用：\nxxx\n\n- 复杂问题用：\n【框架】\n1. xxx\n2. xxx\n3. xxx\n\n【核心点】\n- xxx\n- xxx\n- xxx\n\n【下一步】\nxxx\n\n- 模糊问题用：\n【先确认】\n1. xxx？\n2. xxx？";
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
struct Settings {
    api_url: String,
    api_key: String,
    model: String,
    theme: String,
    last_conversation_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistorySummary {
    id: i64,
    question_preview: String,
    created_at: i64,
    status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct HistoryRecord {
    id: i64,
    conversation_id: i64,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ConversationSummary {
    id: i64,
    title: String,
    mode: String,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct SessionMemory {
    session_goal: String,
    confirmed_facts: Vec<String>,
    constraints: Vec<String>,
    preferences: Vec<String>,
    progress: Vec<String>,
    open_questions: Vec<String>,
    next_action: String,
    key_decisions: Vec<String>,
    risks_or_issues: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KnowledgeNodeSummary {
    id: i64,
    title: String,
    summary: String,
    source_count: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KnowledgeNeighbor {
    node_id: i64,
    title: String,
    summary: String,
    relation_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KnowledgeSourceItem {
    qa_record_id: i64,
    question: String,
    answer: String,
    created_at: i64,
    model: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KnowledgeNodeDetail {
    id: i64,
    title: String,
    summary: String,
    aliases: Vec<String>,
    source_count: i64,
    updated_at: i64,
    sources: Vec<KnowledgeSourceItem>,
    neighbors: Vec<KnowledgeNeighbor>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KnowledgeTaskStatus {
    last_run_at: Option<i64>,
    last_status: String,
    last_error: Option<String>,
    last_processed_qa_id: Option<i64>,
    pending_records: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildKnowledgeMapResult {
    status: String,
    processed_records: usize,
    created_nodes: usize,
    updated_nodes: usize,
    created_edges: usize,
    pending_records: i64,
    last_run_at: i64,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelCallLogEntry {
    timestamp: i64,
    purpose: String,
    model: String,
    api_url: String,
    api_kind: String,
    request_body: serde_json::Value,
    response_status: Option<u16>,
    response_ok: bool,
    response_body: Option<String>,
    error: Option<String>,
}

#[derive(Debug)]
struct KnowledgeTaskStateRow {
    last_run_at: Option<i64>,
    last_status: String,
    last_error: Option<String>,
    last_processed_qa_id: Option<i64>,
}

#[derive(Debug, Clone)]
struct PendingQaRecord {
    id: i64,
    question: String,
    answer: String,
}

#[derive(Debug, Clone)]
struct ClusterRecord {
    id: i64,
    question: String,
    answer: String,
}

#[derive(Debug, Clone)]
struct KnowledgeCluster {
    records: Vec<ClusterRecord>,
    terms: HashSet<String>,
}

#[derive(Debug, Clone)]
struct ExistingKnowledgeNode {
    id: i64,
    title: String,
    normalized_title: String,
    aliases: Vec<String>,
    terms: HashSet<String>,
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

#[derive(Debug, Clone)]
struct MemoryMessage {
    role: String,
    content: String,
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

#[derive(Debug, Deserialize)]
struct KnowledgeExtraction {
    #[serde(
        default,
        alias = "nodeTitle",
        alias = "node_title",
        alias = "name",
        alias = "topic",
        alias = "knowledgeTitle"
    )]
    title: String,
    #[serde(
        default,
        alias = "nodeSummary",
        alias = "node_summary",
        alias = "description",
        alias = "desc",
        alias = "knowledgeSummary"
    )]
    summary: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default, alias = "relatedNodes", alias = "related_nodes")]
    related_nodes: Vec<String>,
    #[serde(default, alias = "prerequisiteNodes", alias = "prerequisite_nodes")]
    prerequisite_nodes: Vec<String>,
    #[serde(default, alias = "confusableNodes", alias = "confusable_nodes")]
    confusable_nodes: Vec<String>,
}

#[derive(Debug)]
enum ApiKind {
    ChatCompletions,
    Responses,
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            settings::load_settings,
            settings::save_settings,
            chat::list_conversations,
            chat::create_conversation,
            chat::delete_conversation,
            chat::update_conversation_mode,
            chat::list_history,
            chat::list_history_records,
            chat::get_history_item,
            chat::ask,
            knowledge::build_knowledge_map,
            knowledge::list_knowledge_nodes,
            knowledge::get_knowledge_node,
            knowledge::list_knowledge_neighbors,
            knowledge::get_knowledge_status
        ])
        .run(tauri::generate_context!())
        .expect("failed to run application");
}
