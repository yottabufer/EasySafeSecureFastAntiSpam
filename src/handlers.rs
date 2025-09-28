use anyhow::Result;
use reqwest::Client;

use crate::{
    config::Config,
    spam_checker::{check_spam_via_openrouter, LlmSpamResult},
    state::{add_user_to_whitelist, increment_ham_counter, is_user_whitelisted, AppState},
    telegram_api::{send_message, Message},
};

/// Обрабатывает входящее сообщение: фильтр спама, ответы и автоворонка в вайтлист.
pub async fn handle_message(
    client: &Client,
    base_url: &str,
    msg: &Message,
    state: &AppState,
    config: &Config,
) -> Result<()> {
    let Some(text) = msg.text.as_deref().filter(|t| !t.trim().is_empty()) else {
        log::info!("Сообщение без текста (игнор) chat_id={}", msg.chat.id);
        return Ok(());
    };
    let text = text.trim();

    let Some(user) = msg.from.as_ref() else {
        log::debug!("Нет отправителя");
        return Ok(());
    };

    if user.is_bot {
        log::info!("Игнор бот-сообщение от id={}", user.id);
        return Ok(());
    }

    let user_id = user.id;

    if is_user_whitelisted(user_id, state).await? {
        return Ok(());
    }

    let Ok(llm) = check_spam_via_openrouter(client, text, &config.openrouter_api_key, &config.openrouter_model).await else {
        log::warn!("Ошибка проверки спама, пропускаем сообщение");
        return Ok(());
    };
    
    log::info!("Оценка спама: {}%, причины: {}", llm.spam_score, llm.notes);
    if llm.spam_score >= config.spam_threshold {
        handle_spam_message(client, base_url, &msg, &llm, config).await?;
    } else {
        handle_ham_message(client, base_url, &msg, user_id, state, &llm, config).await?;
    }

    Ok(())
}

/// Обрабатывает сообщение, классифицированное как спам
async fn handle_spam_message(
    client: &Client,
    base_url: &str,
    msg: &Message,
    llm: &LlmSpamResult,
    config: &Config,
) -> Result<()> {
    let mention = config
        .tag_username
        .as_ref()
        .map(|u| format!("@{u} "))
        .unwrap_or_else(|| String::new());

    if let Err(e) = send_message(
        client,
        base_url,
        msg.chat.id,
        &format!("{mention}СПАМ ({}%). Причина: {}", llm.spam_score, llm.notes),
        Some(msg.message_id),
    )
    .await {
        log::warn!("Ошибка отправки сообщения о спаме: {e}");
    }

    Ok(())
}

/// Обрабатывает сообщение, классифицированное как не-спам
async fn handle_ham_message(
    client: &Client,
    base_url: &str,
    msg: &Message,
    user_id: i64,
    state: &AppState,
    llm: &LlmSpamResult,
    config: &Config,
) -> Result<()> {
    let count = increment_ham_counter(user_id, state).await;

    if config.echo_ham {
        if let Err(e) = send_message(
            client,
            base_url,
            msg.chat.id,
            &format!("HAM ({count} / {}) — {}%", config.ham_threshold, llm.spam_score),
            None,
        )
        .await {
            log::warn!("Ошибка отправки HAM сообщения: {e}");
        }
    }

    if count >= config.ham_threshold {
        add_user_to_whitelist(user_id, state, &config.whitelist_path).await?;
        
        let username_tag = msg
            .from
            .as_ref()
            .and_then(|u| u.username.as_ref())
            .map(|u| format!("@{u}"))
            .unwrap_or_else(|| format!("id {user_id}"));

        if let Err(e) = send_message(
            client,
            base_url,
            msg.chat.id,
            &format!(
                "Пользователь {username_tag} добавлен в белый список после {} корректных сообщений",
                config.ham_threshold
            ),
            None,
        )
        .await {
            log::warn!("Ошибка отправки сообщения о добавлении в вайтлист: {e}");
        }
    }

    Ok(())
}
