use super::*;

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
