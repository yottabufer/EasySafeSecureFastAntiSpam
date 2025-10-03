use std::path::PathBuf;

/// Конфигурация приложения
/// Значения берутся из переменных окружения и дефолтов
#[derive(Debug)]
pub struct Config {
    pub bot_token: String,
    pub whitelist_path: PathBuf,
    pub spam_threshold: u8,
    pub ham_threshold: u32,
    pub tag_username: Option<String>,
    pub ollama_model: String,
    pub notify_user_id: Option<i64>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
            .context("Отсутствует переменная окружения TELEGRAM_BOT_TOKEN")?;

        let whitelist_path = std::env::var("WHITE_USER_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("white_user.txt"));

        let spam_threshold = std::env::var("SPAM_THRESHOLD")
            .unwrap_or("70".to_string())
            .parse()
            .unwrap_or(70);

        let ham_threshold = std::env::var("HAM_WHITELIST_THRESHOLD")
            .unwrap_or("15".to_string())
            .parse()
            .unwrap_or(15);

        let tag_username = std::env::var("TEG_USERNAME")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(|v| v.trim_start_matches('@').to_string());

        let ollama_model = std::env::var("OLLAMA_MODEL")
            .unwrap_or("llama3.2:3b".to_string());

        let notify_user_id = std::env::var("NOTIFY_USER_ID")
            .ok()
            .and_then(|v| v.trim().parse::<i64>().ok());

        Ok(Config {
            bot_token,
            whitelist_path,
            spam_threshold,
            ham_threshold,
            tag_username,
            ollama_model,
            notify_user_id,
        })
    }
}

use anyhow::Context;
