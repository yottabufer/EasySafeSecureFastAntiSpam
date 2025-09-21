use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Результат классификации спама от LLM через OpenRouter.
#[derive(Deserialize, Debug)]
pub struct LlmSpamResult {
    #[serde(default)]
    pub spam_score: u8,
    #[serde(default)]
    pub notes: String,
}

/// Запрос к OpenRouter Chat Completions API.
#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f32,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

/// Запрашивает модель через OpenRouter для оценки спама.
pub async fn check_spam_via_openrouter(
    client: &reqwest::Client,
    text: &str, 
    api_key: &str, 
    model: &str
) -> Result<LlmSpamResult> {
    let system_prompt = r#"Ты — строгий фильтр спама для чатов. Каждое сообщение с подозрительным предложением заработка — спам. Возвращай ТОЛЬКО JSON:
    { "spam_score": <0..100>, "notes": "причины одной строкой" }
    
    Правила:
    - spam_score: целое число. 100 = явный спам.
    - notes: перечисли ВСЕ признаки.
    - Никакого текста вне JSON.
    
    ЖЁСТКИЕ ТРИГГЕРЫ (если один из них — минимум 90):
    - "пиши +", "поставьте +", "напишите «+»" → всегда спам (95+).
    - "@username" в контексте предложения работы/дохода → спам.
    - "удалённая деятельность", "работа онлайн", "доход" без деталей → спам.
    - Указание суммы дохода (особенно >500$) → спам.
    - "возьму", "набираю", "срочно" + люди → спам.
    - Переход в ЛС или к контакту → спам.
    - "легально", "без ставок", "без закладок" — часто маскировка запрещённых схем.
    
    Если совпадает 2+ триггера → 98–100.
    Никаких сомнений. Такие сообщения никогда не бывают легитимными."#;

    let user_prompt = format!(
        "Сообщение для анализа:\n{text}\n\nВерни только JSON с полями spam_score и notes."
    );

    let body = ChatRequest {
        model,
        messages: vec![
            ChatMessage {
                role: "system",
                content: system_prompt,
            },
            ChatMessage {
                role: "user",
                content: &user_prompt,
            },
        ],
        temperature: 0.0,
        max_tokens: 128,
        top_p: Some(0.1),
    };

    let resp = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_else(|e| {
            log::warn!("Не удалось прочитать ответ OpenRouter: {}", e);
            "Ошибка чтения ответа".to_string()
        });
        anyhow::bail!("OpenRouter HTTP {status}: {text}");
    }

    let parsed: serde_json::Value = resp.json().await?;
    let content = parsed
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| {
            log::warn!("OpenRouter вернул пустой ответ");
            "{}".to_string()
        });

    match serde_json::from_str::<LlmSpamResult>(&content) {
        Ok(mut r) => {
            if r.spam_score > 100 {
                r.spam_score = 100;
            }
            Ok(r)
        }
        Err(e) => {
            anyhow::bail!("Некорректный JSON от OpenRouter: {e} | raw: {content}")
        }
    }
}
