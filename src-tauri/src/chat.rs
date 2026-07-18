//! Local conversation history — stored only on disk, never sent anywhere.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub model_id: Option<String>,
    pub system_prompt: String,
    pub messages: Vec<ChatMessage>,
    pub updated_at: u64,
}

fn conversations_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Could not resolve app data dir: {e}"))?;
    let dir = base.join("conversations");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create conversations dir: {e}"))?;
    Ok(dir)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[tauri::command]
pub fn list_conversations(app: AppHandle) -> Result<Vec<Conversation>, String> {
    let dir = conversations_dir(&app)?;
    let mut list = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if let Ok(conv) = serde_json::from_str::<Conversation>(&text) {
            list.push(conv);
        }
    }
    list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(list)
}

#[tauri::command]
pub fn load_conversation(app: AppHandle, id: String) -> Result<Conversation, String> {
    let path = conversations_dir(&app)?.join(format!("{id}.json"));
    let text = fs::read_to_string(&path).map_err(|e| format!("Conversation not found: {e}"))?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_conversation(app: AppHandle, mut conversation: Conversation) -> Result<Conversation, String> {
    if conversation.id.is_empty() {
        conversation.id = Uuid::new_v4().to_string();
    }
    conversation.updated_at = now_secs();
    if conversation.title.trim().is_empty() {
        conversation.title = conversation
            .messages
            .iter()
            .find(|m| m.role == "user")
            .map(|m| {
                let t = m.content.trim();
                if t.chars().count() > 48 {
                    format!("{}…", t.chars().take(48).collect::<String>())
                } else if t.is_empty() {
                    "New chat".into()
                } else {
                    t.to_string()
                }
            })
            .unwrap_or_else(|| "New chat".into());
    }
    let path = conversations_dir(&app)?.join(format!("{}.json", conversation.id));
    let text = serde_json::to_string_pretty(&conversation).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(conversation)
}

#[tauri::command]
pub fn delete_conversation(app: AppHandle, id: String) -> Result<(), String> {
    let path = conversations_dir(&app)?.join(format!("{id}.json"));
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn new_conversation(model_id: Option<String>) -> Conversation {
    Conversation {
        id: Uuid::new_v4().to_string(),
        title: "New chat".into(),
        model_id,
        system_prompt: String::new(),
        messages: Vec::new(),
        updated_at: now_secs(),
    }
}
