use std::path::PathBuf;

/// Конфигурация приложения
#[derive(Debug)]
pub struct Config {
    pub bot_token: String,
    pub whitelist_path: PathBuf,
    pub spam_threshold: u8,
    pub ham_threshold: u32,
    pub echo_ham: bool,
    pub tag_username: Option<String>,
    pub openrouter_api_key: String,
    pub openrouter_model: String,
}

impl Config {
    /// Загружает конфигурацию из переменных окружения
    pub fn from_env() -> anyhow::Result<Self> {
        let bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
            .context("Отсутствует переменная окружения TELEGRAM_BOT_TOKEN")?;

        let whitelist_path = std::env::var("WHITE_USER_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("white_user.txt"));

        let spam_threshold = std::env::var("SPAM_THRESHOLD")
            .unwrap_or_else(|_| "70".to_string())
            .parse()
            .unwrap_or(70);

        let ham_threshold = std::env::var("HAM_WHITELIST_THRESHOLD")
            .unwrap_or_else(|_| "15".to_string())
            .parse()
            .unwrap_or(15);

        let echo_ham = std::env::var("ECHO_HAM")
            .unwrap_or_else(|_| "0".to_string()) == "1";

        let tag_username = std::env::var("TEG_USERNAME")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(|v| v.trim_start_matches('@').to_string());

        let openrouter_api_key = std::env::var("OPENROUTER_API_KEY")
            .context("Отсутствует переменная окружения OPENROUTER_API_KEY")?;

        let openrouter_model = std::env::var("OPENROUTER_MODEL")
            .unwrap_or_else(|_| "qwen/qwen-2.5-coder-32b-instruct".to_string());

        Ok(Config {
            bot_token,
            whitelist_path,
            spam_threshold,
            ham_threshold,
            echo_ham,
            tag_username,
            openrouter_api_key,
            openrouter_model,
        })
    }
}

use anyhow::Context;
