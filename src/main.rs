use anyhow::Result;
use reqwest::Client;
use std::time::Duration;

mod config;
mod handlers;
mod spam_checker;
mod state;
mod telegram_api;

use config::Config;
use state::{AppState, load_whitelist};
use telegram_api::{delete_webhook, get_me, get_updates};

/// Главная функция: инициализация, загрузка конфигурации и запуск бота
#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = Config::from_env()?;
    let whitelist = load_whitelist(&config.whitelist_path).await.unwrap_or_else(|e| {
        log::warn!("Не удалось загрузить вайтлист: {}. Используется пустой список.", e);
        std::collections::HashSet::new()
    });
    let state = AppState::new(whitelist);

    log::info!("Бот запущен. Ожидаю сообщения...");

    run_long_polling(&config, state).await?;

    Ok(())
}

/// Создает HTTP клиент с таймаутами для Telegram API
/// Используется для запросов к Telegram
fn create_client() -> Result<Client> {
    Ok(Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(75))
        .build()?)
}

/// Long polling: получение и обработка обновлений от Telegram
async fn run_long_polling(config: &Config, state: AppState) -> Result<()> {
    let client = create_client()?;
    let base_url = format!("https://api.telegram.org/bot{}", config.bot_token);
    
    delete_webhook(&client, &base_url).await.ok();
    if let Err(err) = get_me(&client, &base_url).await {
        log::warn!("getMe error: {err:?}");
    }
    
    let mut offset: i64 = 0;

    loop {
        let Ok(resp) = get_updates(&client, &base_url, offset).await else {
            log::warn!("getUpdates error, повтор через 2 секунды...");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            continue;
        };

        for upd in resp.result {
            offset = upd.update_id + 1;
            if let Some(msg) = upd.message {
                if let Err(err) = handlers::handle_message(&client, &base_url, &msg, &state, config).await {
                    log::error!("handler error: {err:?}");
                }
            }
        }
    }
}
