use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

/// Структуры для работы с Telegram Bot API
#[derive(Deserialize, Debug)]
pub struct TgUpdate {
    pub update_id: i64,
    pub message: Option<Message>,
}

/// Сообщение Telegram
#[derive(Deserialize, Debug)]
pub struct Message {
    pub message_id: i64,
    pub from: Option<User>,
    pub chat: Chat,
    pub text: Option<String>,
}

/// Пользователь Telegram
#[derive(Deserialize, Debug)]
pub struct User {
    pub id: i64,
    pub is_bot: bool,
    pub username: Option<String>,
}

/// Чат Telegram
#[derive(Deserialize, Debug)]
pub struct Chat {
    pub id: i64,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub r#type: String,
}

/// Обёртка ответа Telegram Bot API
#[derive(Deserialize, Debug)]
pub struct TgResponse<T> { 
    pub result: T 
}


/// Отправляет текстовое сообщение через Telegram Bot API.
pub async fn send_message(
    client: &Client, 
    base_url: &str, 
    chat_id: i64, 
    text: &str, 
    reply_to_message_id: Option<i64>
) -> Result<()> {
    let url: String = format!("{base_url}/sendMessage");
    let mut payload: serde_json::Value = serde_json::json!({ "chat_id": chat_id, "text": text });
    if let Some(mid) = reply_to_message_id {
        payload["reply_to_message_id"] = serde_json::json!(mid);
        payload["allow_sending_without_reply"] = serde_json::json!(true);
    }
    let resp = client.post(&url).json(&payload).send().await?;
    if !resp.status().is_success() {
        log::warn!("sendMessage HTTP {}: {}", resp.status(), resp.text().await.unwrap_or_default());
    }
    Ok(())
}

/// Отключает вебхук у бота, чтобы работал long polling.
pub async fn delete_webhook(client: &Client, base_url: &str) -> Result<()> {
    let url: String = format!("{base_url}/deleteWebhook");
    let _ = client.post(&url).json(&serde_json::json!({"drop_pending_updates": false})).send().await?;
    Ok(())
}

/// Проверяет корректность токена, запрашивая getMe у Telegram Bot API.
pub async fn get_me(client: &Client, base_url: &str) -> Result<()> {
    #[derive(Deserialize)]
    struct Me { 
        id: i64, 
        username: Option<String> 
    }
    let url: String = format!("{base_url}/getMe");
    let resp: reqwest::Response = client.get(&url).send().await?;
    let parsed: TgResponse<Me> = resp.json().await?;
    log::info!("getMe: id={}, username={:?}", parsed.result.id, parsed.result.username);
    Ok(())
}

/// Получает обновления от Telegram Bot API
pub async fn get_updates(
    client: &Client, 
    base_url: &str, 
    offset: i64
) -> Result<TgResponse<Vec<TgUpdate>>> {
    let url: String = format!("{base_url}/getUpdates");
    let resp: reqwest::Response = client
        .post(&url)
        .json(&serde_json::json!({
            "timeout": 60,
            "offset": offset,
            "allowed_updates": ["message"],
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("getUpdates HTTP {}: {}", resp.status(), resp.text().await.unwrap_or_default());
    }

    let parsed: TgResponse<Vec<TgUpdate>> = resp.json().await?;
    Ok(parsed)
}
