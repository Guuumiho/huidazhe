use super::*;

#[tauri::command]
pub(crate) fn get_conversation_map(app: AppHandle, conversation_id: i64) -> Result<ConversationMapGraph, String> {
    let connection = crate::storage::open_database(&app)?;
    load_conversation_map(&connection, conversation_id)
}

#[tauri::command]
pub(crate) fn list_conversation_map_events(
    app: AppHandle,
    conversation_id: i64,
) -> Result<Vec<ConversationMapEvent>, String> {
    let connection = crate::storage::open_database(&app)?;
    let mut statement = connection
        .prepare(
            "SELECT id, conversation_id, qa_record_id, raw_llm_output, applied_operations_json, created_at
             FROM conversation_map_events
             WHERE conversation_id = ?1
             ORDER BY created_at DESC, id DESC
             LIMIT 50",
        )
        .map_err(|error| format!("Failed to read conversation map events: {error}"))?;

    let rows = statement
        .query_map([conversation_id], |row| {
            Ok(ConversationMapEvent {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                qa_record_id: row.get(2)?,
                raw_llm_output: row.get(3)?,
                applied_operations_json: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|error| format!("Failed to read conversation map events: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read conversation map events: {error}"))
}

#[tauri::command]
pub(crate) async fn refresh_conversation_map(
    app: AppHandle,
    conversation_id: i64,
    qa_record_id: i64,
) -> Result<ConversationMapGraph, String> {
    refresh_conversation_map_internal(app.clone(), conversation_id, qa_record_id).await?;
    let connection = crate::storage::open_database(&app)?;
    load_conversation_map(&connection, conversation_id)
}

pub(crate) async fn refresh_conversation_map_internal(
    app: AppHandle,
    conversation_id: i64,
    qa_record_id: i64,
) -> Result<(), String> {
    let connection = crate::storage::open_database(&app)?;
    let qa_row = connection
        .query_row(
            "SELECT question, answer, status
             FROM qa_records
             WHERE id = ?1 AND conversation_id = ?2",
            params![qa_record_id, conversation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("Failed to read conversation map source record: {error}"))?;

    let Some((question, answer, status)) = qa_row else {
        return Ok(());
    };

    if status != "success" || answer.trim().is_empty() {
        return Ok(());
    }

    let settings = crate::settings::load_settings(app.clone())?;
    if settings.api_url.trim().is_empty() || settings.api_key.trim().is_empty() {
        return Ok(());
    }

    let graph = load_conversation_map(&connection, conversation_id)?;
    let system_prompt = "You are a thought-map node and relation extractor. Return valid JSON only, no markdown, no explanation. Prefer user content. Assistant content can only contribute high-value conclusions, reframings, or clarified structure. Only output incremental additions for the current round. If the current round adds no new structure, return {\"new_nodes\":[],\"new_edges\":[]}. In new_nodes, put the primary user-derived theme first when a new user theme is introduced.";
    let user_prompt = build_conversation_map_prompt(&graph, &question, &answer);

    match crate::chat::send_model_text_request(
        &app,
        &settings,
        AUXILIARY_MODEL,
        "conversation_map_update",
        Some(system_prompt),
        &user_prompt,
        &[],
    )
    .await
    {
        Ok(raw_text) => {
            let parsed_text = match crate::chat::parse_model_text(&settings.api_url, &raw_text) {
                Ok(text) => text,
                Err(error) => {
                    let _ = insert_conversation_map_event(
                        &connection,
                        conversation_id,
                        qa_record_id,
                        Some(raw_text),
                        Some(serde_json::json!({"status":"error","message": format!("Failed to parse conversation map response: {error}")}).to_string()),
                    );
                    return Ok(());
                }
            };

            let json_text = match extract_map_json(&parsed_text) {
                Some(text) => text,
                None => {
                    let _ = insert_conversation_map_event(
                        &connection,
                        conversation_id,
                        qa_record_id,
                        Some(parsed_text.clone()),
                        Some(serde_json::json!({"status":"error","message": format!("Conversation map update did not return valid JSON: {}", crate::chat::sanitize_text(&parsed_text, 280))}).to_string()),
                    );
                    return Ok(());
                }
            };

            let update: ConversationMapExtraction = match serde_json::from_str(&json_text) {
                Ok(update) => update,
                Err(error) => {
                    let _ = insert_conversation_map_event(
                        &connection,
                        conversation_id,
                        qa_record_id,
                        Some(parsed_text.clone()),
                        Some(serde_json::json!({"status":"error","message": format!("Failed to parse conversation map JSON: {error}")}).to_string()),
                    );
                    return Ok(());
                }
            };

            let applied_operations_json =
                apply_conversation_map_update(&connection, conversation_id, qa_record_id, &question, &graph, update)?;

            insert_conversation_map_event(
                &connection,
                conversation_id,
                qa_record_id,
                Some(parsed_text),
                Some(applied_operations_json),
            )?;
            Ok(())
        }
        Err(error) => {
            let _ = insert_conversation_map_event(
                &connection,
                conversation_id,
                qa_record_id,
                None,
                Some(serde_json::json!({"status":"error","message": error}).to_string()),
            );
            Ok(())
        }
    }
}

fn build_conversation_map_prompt(graph: &ConversationMapGraph, question: &str, answer: &str) -> String {
    let graph_json = serde_json::json!({
        "nodes": &graph.nodes,
        "edges": &graph.edges,
    })
    .to_string();
    format!(
        "你是“思路图节点/关系提取器”。\n\n任务目标\n根据【用户-AI对话】和【已有思路图 JSON】，抽取本轮对话中“新增的主题节点、节点描述、主题关系”，用于持续完善用户思路发展路径图。\n\n输入\n1. 对话记录：\nuser: 用户本轮输入\nassistant: AI本轮回复\n2. 已有思路图：\n{graph_json}\n\n输出要求\n只输出合法 JSON，不能输出解释、注释、markdown。\n\n输出格式：\n{{\n  \"new_nodes\": [\n    {{\n      \"id\": \"temp_1\",\n      \"label\": \"节点标题\",\n      \"type\": \"目标|子目标|任务|状态|问题定义|方法|原则|产出物|依赖\",\n      \"description\": \"对该节点的简洁描述，说明它在用户思路中的含义\"\n    }}\n  ],\n  \"new_edges\": [\n    {{\n      \"sid\": \"源节点id\",\n      \"tid\": \"目标节点id\",\n      \"type\": \"拆分|导致|支撑|依赖|用于|澄清|转化|对标\"\n    }}\n  ]\n}}\n\n抽取原则\n1. 优先提取用户内容\n主要依据 user 的表述提取节点和关系。\nassistant 只作为辅助参考，且主要只看其中的“结论/判断/重定义”部分。\n不要把 assistant 里的执行建议、时间安排、措辞包装机械地提成节点，除非它是在重新定义用户问题结构。\n2. 只提取“新增信息”\n如果某主题在已有 graph_json 中已存在，且本轮没有新增内涵，不重复输出。\n若用户是在细化已有节点，应新增“更具体的子节点”或“新的关系”，不要重复旧节点。\n若只是换一种说法表达同一主题，不新增。\n3. 节点抽取标准\n仅提取对“思路结构”有价值的主题，优先包括：长期目标、中间目标/子目标、具体任务模块、当前状态/阻碍、对问题的新定义、方法原则、关键产出物、外部参考/依赖对象。\n不要提取：寒暄、情绪修饰词、纯执行细节、没有独立主题意义的句子。\n4. 关系抽取标准\n关系类型：拆分、导致、支撑、依赖、用于、澄清、转化、对标。\n5. AI 结论使用规则\n只有当 assistant 的“结论”是在概括用户隐含状态、重定义用户任务，或比用户原话更结构化且不偏离用户意图时，才可吸收为新节点或关系。若 assistant 只是给建议，不要提取。\n6. 去重与合并\n若新主题明显属于已有节点的组成部分，则优先输出为子节点，并建立合理关系。\n若多个短语本质是同一主题，合并成一个节点。\nlabel 要短，description 要补足含义。\n7. ID 规则\n新节点 id 使用 temp_1、temp_2 ...；new_edges 中允许引用已有节点 id 和新节点 id。\n8. 额外约束\n- 如果当前轮引入了新的用户核心主题，请把它放在 new_nodes 的第一个位置。\n- assistant 衍生节点最多 3 个。\n- 如果没有新增，输出 {{\"new_nodes\":[],\"new_edges\":[]}}。\n\n已有思路图 JSON：\n{graph_json}\n\n本轮对话：\nuser: {question}\nassistant: {answer}"
    )
}

fn load_conversation_map(connection: &Connection, conversation_id: i64) -> Result<ConversationMapGraph, String> {
    let mut node_statement = connection
        .prepare(
            "SELECT id, conversation_id, title, node_type, topic_type, description, status, created_from_record_id, created_at, updated_at
             FROM conversation_map_nodes
             WHERE conversation_id = ?1 AND status = 'active'
             ORDER BY updated_at DESC, created_at ASC, id ASC",
        )
        .map_err(|error| format!("Failed to read conversation map nodes: {error}"))?;

    let nodes = node_statement
        .query_map([conversation_id], |row| {
            Ok(ConversationMapNode {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                label: row.get(2)?,
                node_type: row.get(3)?,
                topic_type: row.get(4)?,
                description: row.get(5)?,
                status: row.get(6)?,
                created_from_record_id: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(|error| format!("Failed to read conversation map nodes: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read conversation map nodes: {error}"))?;

    let mut edge_statement = connection
        .prepare(
            "SELECT id, conversation_id, from_node_id, to_node_id, relation_type
             FROM conversation_map_edges
             WHERE conversation_id = ?1
             ORDER BY id ASC",
        )
        .map_err(|error| format!("Failed to read conversation map edges: {error}"))?;

    let edges = edge_statement
        .query_map([conversation_id], |row| {
            Ok(ConversationMapEdge {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                from_node_id: row.get(2)?,
                to_node_id: row.get(3)?,
                relation_type: row.get(4)?,
            })
        })
        .map_err(|error| format!("Failed to read conversation map edges: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read conversation map edges: {error}"))?;

    Ok(ConversationMapGraph { nodes, edges })
}

fn apply_conversation_map_update(
    connection: &Connection,
    conversation_id: i64,
    qa_record_id: i64,
    question: &str,
    graph: &ConversationMapGraph,
    update: ConversationMapExtraction,
) -> Result<String, String> {
    let now = Utc::now().timestamp_millis();
    let mut node_index = HashMap::new();
    for node in &graph.nodes {
        node_index.insert(node.id, node.clone());
    }
    let mut existing_by_normalized = HashMap::new();
    for node in &graph.nodes {
        existing_by_normalized.insert(normalize_map_label(&node.label), node.id);
    }

    let mut temp_to_real = HashMap::<String, i64>::new();
    let mut user_anchor_node_id = None;
    let mut created_nodes = Vec::new();
    let mut reused_nodes = Vec::new();
    let mut new_nodes = update.new_nodes;

    if new_nodes.is_empty() && graph.nodes.is_empty() {
        new_nodes.push(ConversationMapDraftNode {
            id: "temp_1".to_string(),
            label: fallback_map_title(question),
            r#type: "问题定义".to_string(),
            description: format!("当前对话新引入的核心问题：{}", crate::chat::sanitize_text(question, 48)),
        });
    }

    for (index, draft_node) in new_nodes.iter().enumerate() {
        let label = sanitize_map_title(&draft_node.label);
        if label.is_empty() {
            continue;
        }

        let normalized = normalize_map_label(&label);
        let is_primary_user_node = index == 0;
        let node_id = if let Some(existing_id) = existing_by_normalized.get(&normalized).copied() {
            touch_conversation_map_node(connection, existing_id, now)?;
            update_conversation_map_node_metadata(
                connection,
                existing_id,
                &normalize_topic_type(&draft_node.r#type),
                &sanitize_map_description(&draft_node.description, question),
                now,
            )?;
            if is_primary_user_node {
                promote_conversation_map_node(connection, existing_id, now)?;
                user_anchor_node_id = Some(existing_id);
            }
            reused_nodes.push(existing_id);
            existing_id
        } else {
            let visual_type = if is_primary_user_node { "user" } else { "assistant" };
            let inserted_id = insert_conversation_map_node(
                connection,
                conversation_id,
                &label,
                visual_type,
                &normalize_topic_type(&draft_node.r#type),
                &sanitize_map_description(&draft_node.description, question),
                qa_record_id,
                now,
            )?;
            existing_by_normalized.insert(normalized, inserted_id);
            if is_primary_user_node {
                user_anchor_node_id = Some(inserted_id);
            }
            created_nodes.push(inserted_id);
            inserted_id
        };

        if !draft_node.id.trim().is_empty() {
            temp_to_real.insert(draft_node.id.trim().to_string(), node_id);
        }
    }

    let fallback_anchor_id = user_anchor_node_id.or_else(|| {
        graph.nodes
            .iter()
            .filter(|node| node.node_type == "user")
            .max_by_key(|node| node.updated_at)
            .map(|node| node.id)
    });

    let mut created_edges = Vec::new();
    for draft_edge in update.new_edges {
        let Some(from_node_id) = resolve_graph_node_reference(&draft_edge.sid, &temp_to_real, &node_index) else {
            continue;
        };
        let Some(to_node_id) = resolve_graph_node_reference(&draft_edge.tid, &temp_to_real, &node_index) else {
            continue;
        };
        if from_node_id == to_node_id {
            continue;
        }

        upsert_conversation_map_edge(
            connection,
            conversation_id,
            from_node_id,
            to_node_id,
            &normalize_relation_type(&draft_edge.r#type),
            now,
        )?;
        created_edges.push(serde_json::json!({
            "from": from_node_id,
            "to": to_node_id,
            "type": normalize_relation_type(&draft_edge.r#type),
        }));
    }

    if let Some(anchor_id) = fallback_anchor_id {
        let has_anchor_edge = created_edges.iter().any(|edge| {
            edge.get("from").and_then(|value| value.as_i64()) == Some(anchor_id)
                || edge.get("to").and_then(|value| value.as_i64()) == Some(anchor_id)
        });

        if !has_anchor_edge {
            for created_node_id in &created_nodes {
                if *created_node_id == anchor_id {
                    continue;
                }
                upsert_conversation_map_edge(
                    connection,
                    conversation_id,
                    anchor_id,
                    *created_node_id,
                    "支撑",
                    now,
                )?;
                created_edges.push(serde_json::json!({
                    "from": anchor_id,
                    "to": created_node_id,
                    "type": "支撑",
                }));
            }
        }
    }

    Ok(serde_json::json!({
        "createdNodeIds": created_nodes,
        "reusedNodeIds": reused_nodes,
        "edges": created_edges,
    }).to_string())
}

fn insert_conversation_map_node(
    connection: &Connection,
    conversation_id: i64,
    label: &str,
    node_type: &str,
    topic_type: &str,
    description: &str,
    qa_record_id: i64,
    now: i64,
) -> Result<i64, String> {
    connection
        .execute(
            "INSERT INTO conversation_map_nodes (conversation_id, title, node_type, topic_type, description, status, created_from_record_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6, ?7, ?8)",
            params![conversation_id, label, node_type, topic_type, description, qa_record_id, now, now],
        )
        .map_err(|error| format!("Failed to insert conversation map node: {error}"))?;
    Ok(connection.last_insert_rowid())
}

fn upsert_conversation_map_edge(
    connection: &Connection,
    conversation_id: i64,
    from_node_id: i64,
    to_node_id: i64,
    relation_type: &str,
    now: i64,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT OR IGNORE INTO conversation_map_edges (conversation_id, from_node_id, to_node_id, relation_type, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![conversation_id, from_node_id, to_node_id, relation_type, now],
        )
        .map_err(|error| format!("Failed to insert conversation map edge: {error}"))?;
    Ok(())
}

fn touch_conversation_map_node(connection: &Connection, node_id: i64, now: i64) -> Result<(), String> {
    connection
        .execute(
            "UPDATE conversation_map_nodes SET updated_at = ?1 WHERE id = ?2",
            params![now, node_id],
        )
        .map_err(|error| format!("Failed to touch conversation map node: {error}"))?;
    Ok(())
}

fn promote_conversation_map_node(connection: &Connection, node_id: i64, now: i64) -> Result<(), String> {
    connection
        .execute(
            "UPDATE conversation_map_nodes
             SET node_type = 'user', updated_at = ?1
             WHERE id = ?2",
            params![now, node_id],
        )
        .map_err(|error| format!("Failed to promote conversation map node: {error}"))?;
    Ok(())
}

fn update_conversation_map_node_metadata(
    connection: &Connection,
    node_id: i64,
    topic_type: &str,
    description: &str,
    now: i64,
) -> Result<(), String> {
    connection
        .execute(
            "UPDATE conversation_map_nodes
             SET topic_type = CASE WHEN ?1 <> '' THEN ?1 ELSE topic_type END,
                 description = CASE WHEN ?2 <> '' THEN ?2 ELSE description END,
                 updated_at = ?3
             WHERE id = ?4",
            params![topic_type, description, now, node_id],
        )
        .map_err(|error| format!("Failed to update conversation map node metadata: {error}"))?;
    Ok(())
}

fn insert_conversation_map_event(
    connection: &Connection,
    conversation_id: i64,
    qa_record_id: i64,
    raw_llm_output: Option<String>,
    applied_operations_json: Option<String>,
) -> Result<(), String> {
    let now = Utc::now().timestamp_millis();
    connection
        .execute(
            "INSERT INTO conversation_map_events (conversation_id, qa_record_id, raw_llm_output, applied_operations_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![conversation_id, qa_record_id, raw_llm_output, applied_operations_json, now],
        )
        .map_err(|error| format!("Failed to write conversation map event: {error}"))?;
    Ok(())
}

fn extract_map_json(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(text[start..=end].to_string())
}

fn sanitize_map_title(title: &str) -> String {
    let compact = title
        .replace(['\r', '\n'], " ")
        .replace(['[', ']', '【', '】', '"', '\''], "")
        .trim()
        .to_string();
    compact.chars().take(18).collect::<String>().trim().to_string()
}

fn fallback_map_title(question: &str) -> String {
    let compact = crate::chat::summarize_question(question);
    compact.chars().take(18).collect::<String>().trim().to_string()
}

fn sanitize_map_description(description: &str, question: &str) -> String {
    let compact = description.replace(['\r', '\n'], " ").trim().to_string();
    if compact.is_empty() {
        format!("围绕“{}”形成的新增思路节点。", crate::chat::sanitize_text(question, 24))
    } else {
        crate::chat::sanitize_text(&compact, 120)
    }
}

fn normalize_map_label(label: &str) -> String {
    label
        .trim()
        .to_lowercase()
        .replace([' ', '\n', '\r', '\t', '，', '。', '：', ':'], "")
}

fn normalize_topic_type(topic_type: &str) -> String {
    match topic_type.trim() {
        "目标" | "子目标" | "任务" | "状态" | "问题定义" | "方法" | "原则" | "产出物" | "依赖" => topic_type.trim().to_string(),
        _ => "任务".to_string(),
    }
}

fn normalize_relation_type(relation_type: &str) -> String {
    match relation_type.trim() {
        "拆分" | "导致" | "支撑" | "依赖" | "用于" | "澄清" | "转化" | "对标" => relation_type.trim().to_string(),
        _ => "支撑".to_string(),
    }
}

fn resolve_graph_node_reference(
    reference: &str,
    temp_to_real: &HashMap<String, i64>,
    node_index: &HashMap<i64, ConversationMapNode>,
) -> Option<i64> {
    let trimmed = reference.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(id) = temp_to_real.get(trimmed).copied() {
        return Some(id);
    }
    let parsed = trimmed.parse::<i64>().ok()?;
    node_index.contains_key(&parsed).then_some(parsed)
}

#[tauri::command]
pub(crate) async fn build_knowledge_map(app: AppHandle) -> Result<BuildKnowledgeMapResult, String> {
    build_knowledge_map_internal(app, true).await
}

#[tauri::command]
pub(crate) fn list_knowledge_nodes(app: AppHandle) -> Result<Vec<KnowledgeNodeSummary>, String> {
    let connection = crate::storage::open_database(&app)?;
    let mut statement = connection
        .prepare(
            "SELECT n.id, n.title, n.summary, n.updated_at, COUNT(s.qa_record_id) AS source_count
             FROM knowledge_nodes n
             LEFT JOIN knowledge_sources s ON s.node_id = n.id
             GROUP BY n.id, n.title, n.summary, n.updated_at
             ORDER BY source_count DESC, n.updated_at DESC, n.id DESC",
        )
        .map_err(|error| format!("Failed to read knowledge nodes: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(KnowledgeNodeSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                summary: row.get(2)?,
                updated_at: row.get(3)?,
                source_count: row.get(4)?,
            })
        })
        .map_err(|error| format!("Failed to read knowledge nodes: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read knowledge nodes: {error}"))
}

#[tauri::command]
pub(crate) fn get_knowledge_node(app: AppHandle, id: i64) -> Result<KnowledgeNodeDetail, String> {
    let connection = crate::storage::open_database(&app)?;
    let node_row = connection
        .query_row(
            "SELECT n.id, n.title, n.summary, n.aliases_json, n.updated_at, COUNT(s.qa_record_id) AS source_count
             FROM knowledge_nodes n
             LEFT JOIN knowledge_sources s ON s.node_id = n.id
             WHERE n.id = ?1
             GROUP BY n.id, n.title, n.summary, n.aliases_json, n.updated_at",
            [id],
            |row| {
                let aliases_json: String = row.get(3)?;
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    decode_aliases(&aliases_json),
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .map_err(|error| format!("Failed to read knowledge node: {error}"))?;

    let sources = load_node_sources(&connection, id)?;
    let neighbors = load_node_neighbors(&connection, id)?;

    Ok(KnowledgeNodeDetail {
        id: node_row.0,
        title: node_row.1,
        summary: node_row.2,
        aliases: node_row.3,
        updated_at: node_row.4,
        source_count: node_row.5,
        sources,
        neighbors,
    })
}

#[tauri::command]
pub(crate) fn list_knowledge_neighbors(app: AppHandle, id: i64) -> Result<Vec<KnowledgeNeighbor>, String> {
    let connection = crate::storage::open_database(&app)?;
    load_node_neighbors(&connection, id)
}

#[tauri::command]
pub(crate) fn get_knowledge_status(app: AppHandle) -> Result<KnowledgeTaskStatus, String> {
    let connection = crate::storage::open_database(&app)?;
    let pending_records = count_pending_qa_records(&connection)?;
    let state = read_knowledge_task_state(&connection)?;

    Ok(KnowledgeTaskStatus {
        last_run_at: state.last_run_at,
        last_status: state.last_status,
        last_error: state.last_error,
        last_processed_qa_id: state.last_processed_qa_id,
        pending_records,
    })
}

pub(crate) async fn build_knowledge_map_internal(app: AppHandle, force: bool) -> Result<BuildKnowledgeMapResult, String> {
    let now = Utc::now().timestamp_millis();
    let connection = crate::storage::open_database(&app)?;
    let pending_before = count_pending_qa_records(&connection)?;
    let task_state = read_knowledge_task_state(&connection)?;

    if !force {
        if let Some(last_run_at) = task_state.last_run_at {
            if now - last_run_at < KNOWLEDGE_CHECK_INTERVAL_MS {
                return Ok(BuildKnowledgeMapResult {
                    status: "skipped".to_string(),
                    processed_records: 0,
                    created_nodes: 0,
                    updated_nodes: 0,
                    created_edges: 0,
                    pending_records: pending_before,
                    last_run_at,
                    message: "Hourly knowledge check skipped because the interval has not elapsed yet.".to_string(),
                });
            }
        }
    }

    if pending_before == 0 {
        upsert_knowledge_task_state(&connection, now, "success", None, task_state.last_processed_qa_id)?;
        return Ok(BuildKnowledgeMapResult {
            status: "success".to_string(),
            processed_records: 0,
            created_nodes: 0,
            updated_nodes: 0,
            created_edges: 0,
            pending_records: 0,
            last_run_at: now,
            message: "No new question records needed to be organized.".to_string(),
        });
    }

    let settings = crate::settings::load_settings(app.clone())?;
    if settings.api_url.trim().is_empty() || settings.api_key.trim().is_empty() {
        upsert_knowledge_task_state(
            &connection,
            now,
            "waiting_settings",
            Some("API settings are required before the knowledge map can be built."),
            task_state.last_processed_qa_id,
        )?;
        return Ok(BuildKnowledgeMapResult {
            status: "waiting_settings".to_string(),
            processed_records: 0,
            created_nodes: 0,
            updated_nodes: 0,
            created_edges: 0,
            pending_records: pending_before,
            last_run_at: now,
            message: "Knowledge map build skipped because API settings are missing.".to_string(),
        });
    }

    let pending_records = fetch_pending_qa_records(&connection, KNOWLEDGE_BATCH_LIMIT)?;
    let clusters = cluster_pending_records(&pending_records);
    let model = KNOWLEDGE_MODEL.to_string();

    let mut created_nodes = 0usize;
    let mut updated_nodes = 0usize;
    let mut created_edges = 0usize;
    let mut processed_record_ids = Vec::new();
    let mut errors = Vec::new();
    let mut existing_nodes = load_existing_knowledge_nodes(&connection)?;
    let mut title_index = build_node_index(&existing_nodes);

    for cluster in clusters {
        match synthesize_cluster(&app, &settings, &model, &cluster, &existing_nodes).await {
            Ok(extraction) => {
                let node_result = upsert_knowledge_node(
                    &connection,
                    &mut existing_nodes,
                    &mut title_index,
                    extraction.title.clone(),
                    extraction.summary.clone(),
                    extraction.aliases.clone(),
                    now,
                )?;

                if node_result.created {
                    created_nodes += 1;
                } else {
                    updated_nodes += 1;
                }

                for record in &cluster.records {
                    link_record_to_knowledge_node(&connection, node_result.node_id, record.id, now)?;
                    processed_record_ids.push(record.id);
                }

                created_edges += create_edges_for_extraction(
                    &connection,
                    node_result.node_id,
                    &extraction,
                    &title_index,
                    now,
                )?;
            }
            Err(error) => {
                let snippet = cluster
                    .records
                    .first()
                    .map(|record| crate::chat::summarize_question(&record.question))
                    .unwrap_or_else(|| "unknown cluster".to_string());
                errors.push(format!("{snippet}: {error}"));
            }
        }
    }

    let last_processed_qa_id = processed_record_ids.into_iter().max().or(task_state.last_processed_qa_id);
    let pending_after = count_pending_qa_records(&connection)?;
    let status = if errors.is_empty() {
        "success"
    } else if pending_after < pending_before {
        "partial_failure"
    } else {
        "error"
    };

    let message = if errors.is_empty() {
        format!(
            "Organized {} knowledge records into {} created nodes and {} updated nodes.",
            pending_before - pending_after,
            created_nodes,
            updated_nodes
        )
    } else {
        format!(
            "Knowledge organization finished with {} issue(s). {} record(s) remain pending.",
            errors.len(),
            pending_after
        )
    };

    let joined_error = if errors.is_empty() {
        None
    } else {
        Some(errors.join(" | "))
    };

    upsert_knowledge_task_state(
        &connection,
        now,
        status,
        joined_error.as_deref(),
        last_processed_qa_id,
    )?;

    Ok(BuildKnowledgeMapResult {
        status: status.to_string(),
        processed_records: (pending_before - pending_after).max(0) as usize,
        created_nodes,
        updated_nodes,
        created_edges,
        pending_records: pending_after,
        last_run_at: now,
        message,
    })
}

async fn synthesize_cluster(
    app: &AppHandle,
    settings: &Settings,
    model: &str,
    cluster: &KnowledgeCluster,
    existing_nodes: &[ExistingKnowledgeNode],
) -> Result<KnowledgeExtraction, String> {
    let candidates = select_candidate_node_titles(&cluster.terms, existing_nodes);
    let records_blob = cluster
        .records
        .iter()
        .map(|record| {
            format!(
                "- Q{}:\n  question: {}\n  answer: {}",
                record.id,
                crate::chat::sanitize_text(&record.question, 240),
                crate::chat::sanitize_text(&record.answer, 360)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let candidates_blob = if candidates.is_empty() {
        "[]".to_string()
    } else {
        serde_json::to_string(&candidates).unwrap_or_else(|_| "[]".to_string())
    };

    let system_prompt = "You organize developer Q&A into personal knowledge nodes. Output JSON only. Keep names concise. Use Chinese when the source questions are Chinese. Relationships must only reference titles from the provided existing candidate titles.";
    let user_prompt = format!(
        "Turn this cluster of related Q&A records into one knowledge node.\n\
         Return JSON with keys: title, summary, aliases, relatedNodes, prerequisiteNodes, confusableNodes.\n\
         - summary must be one concise sentence.\n\
         - aliases should be short phrases.\n\
         - relationship arrays may only contain titles from ExistingCandidateTitles.\n\
         - if there is no confident relation, return an empty array.\n\
         ExistingCandidateTitles: {candidates_blob}\n\
         ClusterRecords:\n{records_blob}"
    );

    let raw_text = crate::chat::send_model_text_request(
        app,
        settings,
        model,
        "knowledge_extraction",
        Some(system_prompt),
        &user_prompt,
        &[],
    )
    .await?;
    let json_text = extract_json_object(&raw_text).ok_or_else(|| {
        format!(
            "Knowledge extraction did not return a valid JSON object. Raw response: {}",
            crate::chat::sanitize_text(&raw_text, 280)
        )
    })?;

    let mut extraction: KnowledgeExtraction =
        serde_json::from_str(&json_text).map_err(|error| format!("Failed to parse extraction JSON: {error}"))?;

    extraction.title = extraction.title.trim().to_string();
    extraction.summary = extraction.summary.trim().to_string();
    extraction.aliases = normalize_title_list(&extraction.aliases);
    extraction.related_nodes = normalize_title_list(&extraction.related_nodes);
    extraction.prerequisite_nodes = normalize_title_list(&extraction.prerequisite_nodes);
    extraction.confusable_nodes = normalize_title_list(&extraction.confusable_nodes);

    if extraction.title.is_empty() {
        extraction.title = crate::chat::summarize_question(
            &cluster
                .records
                .first()
                .map(|record| record.question.as_str())
                .unwrap_or("未命名知识点"),
        );
    }
    if extraction.summary.is_empty() {
        extraction.summary = extraction.title.clone();
    }

    Ok(extraction)
}

fn cluster_pending_records(records: &[PendingQaRecord]) -> Vec<KnowledgeCluster> {
    let mut clusters: Vec<KnowledgeCluster> = Vec::new();

    for record in records {
        let mut terms = extract_candidate_terms(&format!("{} {}", record.question, record.answer));
        if terms.is_empty() {
            terms.insert(normalize_title(&crate::chat::summarize_question(&record.question)));
        }

        let mut best_cluster_index = None;
        let mut best_score = 0usize;

        for (index, cluster) in clusters.iter().enumerate() {
            let overlap = term_overlap_count(&terms, &cluster.terms);
            if overlap > best_score {
                best_score = overlap;
                best_cluster_index = Some(index);
            }
        }

        let cluster_record = ClusterRecord {
            id: record.id,
            question: record.question.clone(),
            answer: record.answer.clone(),
        };

        if let Some(index) = best_cluster_index {
            let union_size = terms.union(&clusters[index].terms).count().max(1);
            let overlap_ratio = best_score as f32 / union_size as f32;
            if best_score >= 2 || overlap_ratio >= 0.35 {
                clusters[index].records.push(cluster_record);
                clusters[index].terms.extend(terms);
                continue;
            }
        }

        clusters.push(KnowledgeCluster {
            records: vec![cluster_record],
            terms,
        });
    }

    clusters
}

fn extract_candidate_terms(text: &str) -> HashSet<String> {
    let mut terms = HashSet::new();
    let mut ascii_buffer = String::new();
    let mut cjk_buffer = String::new();

    for character in text.chars() {
        if character.is_ascii_alphanumeric() {
            if !cjk_buffer.is_empty() {
                flush_cjk_terms(&mut terms, &mut cjk_buffer);
            }
            ascii_buffer.push(character.to_ascii_lowercase());
        } else if is_cjk(character) {
            if !ascii_buffer.is_empty() {
                flush_ascii_terms(&mut terms, &mut ascii_buffer);
            }
            cjk_buffer.push(character);
        } else {
            if !ascii_buffer.is_empty() {
                flush_ascii_terms(&mut terms, &mut ascii_buffer);
            }
            if !cjk_buffer.is_empty() {
                flush_cjk_terms(&mut terms, &mut cjk_buffer);
            }
        }
    }

    if !ascii_buffer.is_empty() {
        flush_ascii_terms(&mut terms, &mut ascii_buffer);
    }
    if !cjk_buffer.is_empty() {
        flush_cjk_terms(&mut terms, &mut cjk_buffer);
    }

    terms.retain(|term| term.len() >= 2);
    terms
}

fn flush_ascii_terms(terms: &mut HashSet<String>, buffer: &mut String) {
    let term = buffer.trim().to_string();
    if term.len() >= 2 {
        terms.insert(term);
    }
    buffer.clear();
}

fn flush_cjk_terms(terms: &mut HashSet<String>, buffer: &mut String) {
    let segment = buffer.trim().to_string();
    if segment.chars().count() >= 2 {
        terms.insert(segment.clone());
        let chars: Vec<char> = segment.chars().collect();
        if chars.len() <= 8 {
            for width in 2..=3 {
                if chars.len() >= width {
                    for start in 0..=(chars.len() - width) {
                        let slice: String = chars[start..start + width].iter().collect();
                        terms.insert(slice);
                    }
                }
            }
        }
    }
    buffer.clear();
}

fn is_cjk(character: char) -> bool {
    matches!(
        character as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF | 0x2A700..=0x2B73F
    )
}

fn term_overlap_count(left: &HashSet<String>, right: &HashSet<String>) -> usize {
    left.intersection(right).count()
}

fn select_candidate_node_titles(cluster_terms: &HashSet<String>, nodes: &[ExistingKnowledgeNode]) -> Vec<String> {
    let mut scored = nodes
        .iter()
        .filter_map(|node| {
            let score = term_overlap_count(cluster_terms, &node.terms);
            if score == 0 {
                None
            } else {
                Some((score, node.title.clone()))
            }
        })
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    scored.into_iter().take(6).map(|(_, title)| title).collect()
}

fn load_existing_knowledge_nodes(connection: &Connection) -> Result<Vec<ExistingKnowledgeNode>, String> {
    let mut statement = connection
        .prepare("SELECT id, title, normalized_title, summary, aliases_json FROM knowledge_nodes")
        .map_err(|error| format!("Failed to read knowledge nodes: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            let aliases_json: String = row.get(4)?;
            let title: String = row.get(1)?;
            let summary: String = row.get(3)?;
            let aliases = decode_aliases(&aliases_json);
            let mut term_text = format!("{title} {summary}");
            if !aliases.is_empty() {
                term_text.push(' ');
                term_text.push_str(&aliases.join(" "));
            }

            Ok(ExistingKnowledgeNode {
                id: row.get(0)?,
                title,
                normalized_title: row.get(2)?,
                aliases,
                terms: extract_candidate_terms(&term_text),
            })
        })
        .map_err(|error| format!("Failed to read knowledge nodes: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read knowledge nodes: {error}"))
}

fn build_node_index(nodes: &[ExistingKnowledgeNode]) -> HashMap<String, i64> {
    let mut index = HashMap::new();
    for node in nodes {
        index.insert(node.normalized_title.clone(), node.id);
        for alias in &node.aliases {
            index.insert(normalize_title(alias), node.id);
        }
    }
    index
}

struct UpsertKnowledgeNodeResult {
    node_id: i64,
    created: bool,
}

fn upsert_knowledge_node(
    connection: &Connection,
    existing_nodes: &mut Vec<ExistingKnowledgeNode>,
    title_index: &mut HashMap<String, i64>,
    title: String,
    summary: String,
    aliases: Vec<String>,
    now: i64,
) -> Result<UpsertKnowledgeNodeResult, String> {
    let normalized_title = normalize_title(&title);
    let mut merged_aliases = normalize_title_list(&aliases);
    merged_aliases.retain(|alias| normalize_title(alias) != normalized_title);

    if let Some(existing_id) = resolve_node_id(title_index, &title, &merged_aliases) {
        let existing_aliases_json: Option<String> = connection
            .query_row(
                "SELECT aliases_json FROM knowledge_nodes WHERE id = ?1",
                [existing_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("Failed to read knowledge node aliases: {error}"))?;

        let mut stored_aliases = existing_aliases_json
            .map(|value| decode_aliases(&value))
            .unwrap_or_default();
        stored_aliases.extend(merged_aliases.clone());
        stored_aliases = normalize_title_list(&stored_aliases);
        let stored_aliases_json = encode_aliases(&stored_aliases)?;

        connection
            .execute(
                "UPDATE knowledge_nodes
                 SET title = ?2, normalized_title = ?3, summary = ?4, aliases_json = ?5, updated_at = ?6
                 WHERE id = ?1",
                params![existing_id, title, normalized_title, summary, stored_aliases_json, now],
            )
            .map_err(|error| format!("Failed to update knowledge node: {error}"))?;

        if let Some(index) = existing_nodes.iter().position(|node| node.id == existing_id) {
            existing_nodes[index] = ExistingKnowledgeNode {
                id: existing_id,
                title: title.clone(),
                normalized_title: normalized_title.clone(),
                aliases: stored_aliases.clone(),
                terms: extract_candidate_terms(&format!("{} {} {}", title, summary, stored_aliases.join(" "))),
            };
        }
        title_index.insert(normalized_title.clone(), existing_id);
        for alias in stored_aliases {
            title_index.insert(normalize_title(&alias), existing_id);
        }

        Ok(UpsertKnowledgeNodeResult {
            node_id: existing_id,
            created: false,
        })
    } else {
        let aliases_json = encode_aliases(&merged_aliases)?;
        connection
            .execute(
                "INSERT INTO knowledge_nodes (title, normalized_title, summary, aliases_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![title, normalized_title, summary, aliases_json, now, now],
            )
            .map_err(|error| format!("Failed to create knowledge node: {error}"))?;

        let node_id = connection.last_insert_rowid();
        existing_nodes.push(ExistingKnowledgeNode {
            id: node_id,
            title: title.clone(),
            normalized_title: normalized_title.clone(),
            aliases: merged_aliases.clone(),
            terms: extract_candidate_terms(&format!("{} {} {}", title, summary, merged_aliases.join(" "))),
        });
        title_index.insert(normalized_title.clone(), node_id);
        for alias in merged_aliases {
            title_index.insert(normalize_title(&alias), node_id);
        }

        Ok(UpsertKnowledgeNodeResult {
            node_id,
            created: true,
        })
    }
}

fn resolve_node_id(title_index: &HashMap<String, i64>, title: &str, aliases: &[String]) -> Option<i64> {
    let normalized_title = normalize_title(title);
    if let Some(id) = title_index.get(&normalized_title) {
        return Some(*id);
    }

    aliases
        .iter()
        .find_map(|alias| title_index.get(&normalize_title(alias)).copied())
}

fn create_edges_for_extraction(
    connection: &Connection,
    node_id: i64,
    extraction: &KnowledgeExtraction,
    title_index: &HashMap<String, i64>,
    now: i64,
) -> Result<usize, String> {
    let mut count = 0usize;
    count += create_edges_for_titles(connection, node_id, &extraction.related_nodes, "related", title_index, now)?;
    count += create_edges_for_titles(
        connection,
        node_id,
        &extraction.prerequisite_nodes,
        "prerequisite",
        title_index,
        now,
    )?;
    count += create_edges_for_titles(
        connection,
        node_id,
        &extraction.confusable_nodes,
        "confusable",
        title_index,
        now,
    )?;
    Ok(count)
}

fn create_edges_for_titles(
    connection: &Connection,
    node_id: i64,
    titles: &[String],
    relation_type: &str,
    title_index: &HashMap<String, i64>,
    now: i64,
) -> Result<usize, String> {
    let mut created = 0usize;

    for title in titles {
        if let Some(target_id) = title_index.get(&normalize_title(title)).copied() {
            if target_id == node_id {
                continue;
            }

            let changed = connection
                .execute(
                    "INSERT OR IGNORE INTO knowledge_edges (from_node_id, to_node_id, relation_type, created_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![node_id, target_id, relation_type, now],
                )
                .map_err(|error| format!("Failed to create knowledge edge: {error}"))?;

            created += changed as usize;
        }
    }

    Ok(created)
}

fn link_record_to_knowledge_node(
    connection: &Connection,
    node_id: i64,
    qa_record_id: i64,
    created_at: i64,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT OR IGNORE INTO knowledge_sources (node_id, qa_record_id, created_at)
             VALUES (?1, ?2, ?3)",
            params![node_id, qa_record_id, created_at],
        )
        .map_err(|error| format!("Failed to link knowledge source: {error}"))?;
    Ok(())
}

fn fetch_pending_qa_records(connection: &Connection, limit: usize) -> Result<Vec<PendingQaRecord>, String> {
    let mut statement = connection
        .prepare(
            "SELECT q.id, q.question, q.answer
             FROM qa_records q
             WHERE q.status = 'success'
               AND q.answer <> ''
               AND NOT EXISTS (
                 SELECT 1
                 FROM knowledge_sources s
                 WHERE s.qa_record_id = q.id
               )
             ORDER BY q.id ASC
             LIMIT ?1",
        )
        .map_err(|error| format!("Failed to read pending knowledge records: {error}"))?;

    let rows = statement
        .query_map([limit as i64], |row| {
            Ok(PendingQaRecord {
                id: row.get(0)?,
                question: row.get(1)?,
                answer: row.get(2)?,
            })
        })
        .map_err(|error| format!("Failed to read pending knowledge records: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read pending knowledge records: {error}"))
}

fn count_pending_qa_records(connection: &Connection) -> Result<i64, String> {
    connection
        .query_row(
            "SELECT COUNT(*)
             FROM qa_records q
             WHERE q.status = 'success'
               AND q.answer <> ''
               AND NOT EXISTS (
                 SELECT 1
                 FROM knowledge_sources s
                 WHERE s.qa_record_id = q.id
               )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Failed to count pending knowledge records: {error}"))
}

fn read_knowledge_task_state(connection: &Connection) -> Result<KnowledgeTaskStateRow, String> {
    let state = connection
        .query_row(
            "SELECT last_run_at, last_status, last_error, last_processed_qa_id
             FROM knowledge_task_state
             WHERE task_name = ?1",
            [KNOWLEDGE_TASK_NAME],
            |row| {
                Ok(KnowledgeTaskStateRow {
                    last_run_at: row.get(0)?,
                    last_status: row.get(1)?,
                    last_error: row.get(2)?,
                    last_processed_qa_id: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("Failed to read knowledge task state: {error}"))?;

    Ok(state.unwrap_or(KnowledgeTaskStateRow {
        last_run_at: None,
        last_status: "idle".to_string(),
        last_error: None,
        last_processed_qa_id: None,
    }))
}

fn upsert_knowledge_task_state(
    connection: &Connection,
    last_run_at: i64,
    last_status: &str,
    last_error: Option<&str>,
    last_processed_qa_id: Option<i64>,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO knowledge_task_state (task_name, last_run_at, last_status, last_error, last_processed_qa_id)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(task_name) DO UPDATE SET
               last_run_at = excluded.last_run_at,
               last_status = excluded.last_status,
               last_error = excluded.last_error,
               last_processed_qa_id = excluded.last_processed_qa_id",
            params![
                KNOWLEDGE_TASK_NAME,
                last_run_at,
                last_status,
                last_error,
                last_processed_qa_id
            ],
        )
        .map_err(|error| format!("Failed to write knowledge task state: {error}"))?;

    Ok(())
}

fn load_node_sources(connection: &Connection, node_id: i64) -> Result<Vec<KnowledgeSourceItem>, String> {
    let mut statement = connection
        .prepare(
            "SELECT q.id, q.question, q.answer, q.created_at, q.model
             FROM knowledge_sources s
             INNER JOIN qa_records q ON q.id = s.qa_record_id
             WHERE s.node_id = ?1
             ORDER BY q.created_at DESC, q.id DESC",
        )
        .map_err(|error| format!("Failed to read knowledge node sources: {error}"))?;

    let rows = statement
        .query_map([node_id], |row| {
            Ok(KnowledgeSourceItem {
                qa_record_id: row.get(0)?,
                question: row.get(1)?,
                answer: row.get(2)?,
                created_at: row.get(3)?,
                model: row.get(4)?,
            })
        })
        .map_err(|error| format!("Failed to read knowledge node sources: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read knowledge node sources: {error}"))
}

fn load_node_neighbors(connection: &Connection, node_id: i64) -> Result<Vec<KnowledgeNeighbor>, String> {
    let mut statement = connection
        .prepare(
            "SELECT e.to_node_id, n.title, n.summary, e.relation_type
             FROM knowledge_edges e
             INNER JOIN knowledge_nodes n ON n.id = e.to_node_id
             WHERE e.from_node_id = ?1
             ORDER BY n.updated_at DESC, n.id DESC",
        )
        .map_err(|error| format!("Failed to read knowledge neighbors: {error}"))?;

    let rows = statement
        .query_map([node_id], |row| {
            Ok(KnowledgeNeighbor {
                node_id: row.get(0)?,
                title: row.get(1)?,
                summary: row.get(2)?,
                relation_type: row.get(3)?,
            })
        })
        .map_err(|error| format!("Failed to read knowledge neighbors: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to read knowledge neighbors: {error}"))
}

fn extract_json_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(text[start..=end].to_string())
}

fn normalize_title(input: &str) -> String {
    input
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_title_list(values: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .filter_map(|value| {
            let normalized = normalize_title(value);
            if seen.insert(normalized) {
                Some(value.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn encode_aliases(aliases: &[String]) -> Result<String, String> {
    serde_json::to_string(aliases).map_err(|error| format!("Failed to serialize aliases: {error}"))
}

fn decode_aliases(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}
