use super::*;

fn empty_conversation_map() -> ConversationMapGraph {
    ConversationMapGraph {
        nodes: Vec::new(),
        edges: Vec::new(),
    }
}

#[tauri::command]
pub(crate) fn get_conversation_map(
    _app: AppHandle,
    _conversation_id: i64,
) -> Result<ConversationMapGraph, String> {
    Ok(empty_conversation_map())
}

#[tauri::command]
pub(crate) fn list_conversation_map_events(
    _app: AppHandle,
    _conversation_id: i64,
) -> Result<Vec<ConversationMapEvent>, String> {
    Ok(Vec::new())
}

#[tauri::command]
pub(crate) async fn refresh_conversation_map(
    _app: AppHandle,
    _conversation_id: i64,
    _qa_record_id: i64,
) -> Result<ConversationMapGraph, String> {
    Ok(empty_conversation_map())
}

#[tauri::command]
pub(crate) async fn build_knowledge_map(_app: AppHandle) -> Result<BuildKnowledgeMapResult, String> {
    Ok(BuildKnowledgeMapResult {
        status: "disabled".to_string(),
        processed_records: 0,
        created_nodes: 0,
        updated_nodes: 0,
        created_edges: 0,
        pending_records: 0,
        last_run_at: Utc::now().timestamp_millis(),
        message: "Knowledge map V1 is disabled while the thought-chain V2 module is redesigned.".to_string(),
    })
}

#[tauri::command]
pub(crate) fn list_knowledge_nodes(_app: AppHandle) -> Result<Vec<KnowledgeNodeSummary>, String> {
    Ok(Vec::new())
}

#[tauri::command]
pub(crate) fn get_knowledge_node(_app: AppHandle, id: i64) -> Result<KnowledgeNodeDetail, String> {
    Err(format!(
        "Knowledge map V1 is disabled while the thought-chain V2 module is redesigned. Node {id} is unavailable."
    ))
}

#[tauri::command]
pub(crate) fn list_knowledge_neighbors(_app: AppHandle, _id: i64) -> Result<Vec<KnowledgeNeighbor>, String> {
    Ok(Vec::new())
}

#[tauri::command]
pub(crate) fn get_knowledge_status(_app: AppHandle) -> Result<KnowledgeTaskStatus, String> {
    Ok(KnowledgeTaskStatus {
        last_run_at: None,
        last_status: "disabled".to_string(),
        last_error: None,
        last_processed_qa_id: None,
        pending_records: 0,
    })
}
