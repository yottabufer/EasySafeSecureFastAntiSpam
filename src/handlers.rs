use anyhow::Result;
use reqwest::Client;

use crate::{
    config::Config,
    spam_checker::check_spam_via_ollama,
    state::{add_user_to_whitelist, increment_ham_counter, is_user_whitelisted, AppState},
    telegram_api::{send_message, Message},
};

/// Основная функция обработки сообщений:
/// 1. Проверяет входящее сообщение на спам через Ollama
/// 2. Для спама — отправляет предупреждение в чат
/// 3. Для не-спама — увеличивает счётчик и добавляет пользователя в вайтлист при достижении порога
pub async fn handle_message(
    client: &Client,
    base_url: &str,
    msg: &Message,
    state: &AppState,
    config: &Config,
) -> Result<()> {
    // Проверяем наличие текста
    let Some(text) = msg.text.as_deref().filter(|t| !t.trim().is_empty()) else {
        return Ok(());
    };
    let text = text.trim();
    let truncated_text: String = text.chars().take(250).collect();

    // Проверяем отправителя
    let Some(user) = msg.from.as_ref() else {
        return Ok(());
    };

    if user.is_bot {
        return Ok(());
    }

    let user_id = user.id;

    // Пропускаем пользователей из вайтлиста
    if is_user_whitelisted(user_id, state).await? {
        log::debug!("Пользователь {} в белом списке", user_id);
        return Ok(());
    }

    // Проверяем сообщение через Ollama
    let llm = match check_spam_via_ollama(client, &truncated_text, "http://127.0.0.1:11434", &config.ollama_model).await {
        Ok(v) => v,
        Err(err) => {
            log::warn!("Ошибка проверки спама: {err:?}");
            return Ok(());
        }
    };
    
    log::info!("Оценка спама: {}%, причины: {}", llm.spam_score, llm.notes);
    
    if llm.spam_score >= config.spam_threshold {
        // Сообщение определено как спам
        let mention = config.tag_username
            .as_ref()
            .map(|u| format!("@{u} "))
            .unwrap_or_default();

        send_message(
            client,
            base_url,
            msg.chat.id,
            &format!("{mention}СПАМ ({}%). Причина: {}", llm.spam_score, llm.notes),
            Some(msg.message_id),
        ).await.ok();
    } else {
        // Сообщение не спам - увеличиваем счетчик
        let count = increment_ham_counter(user_id, state).await;

        // Добавляем в вайтлист после достижения порога
        if count >= config.ham_threshold {
            add_user_to_whitelist(user_id, state, &config.whitelist_path).await?;
            
            let username_tag = msg.from
                .as_ref()
                .and_then(|u| u.username.as_ref())
                .map(|u| format!("@{u}"))
                .unwrap_or_else(|| format!("id {user_id}"));

            let target_chat = config.notify_user_id.unwrap_or(msg.chat.id);
            send_message(
                client,
                base_url,
                target_chat,
                &format!(
                    "Пользователь {username_tag} добавлен в белый список после {} корректных сообщений",
                    config.ham_threshold
                ),
                None,
            ).await.ok();
        }
    }

    Ok(())
}

