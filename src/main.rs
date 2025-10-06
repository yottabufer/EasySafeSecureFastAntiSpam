use anyhow::Result;
use clap::Parser;
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;

mod config;
mod handlers;
mod spam_checker;
mod state;
mod telegram_api;
mod kick_deleted;

use config::Config;
use state::{AppState, load_whitelist};
use telegram_api::{delete_webhook, get_me, get_updates};

#[derive(Parser)]
#[command(name = "tg_anti_spam")]
#[command(about = "Telegram Anti-Spam Bot and utilities")]
enum Args {
    /// Запустить бота для фильтрации спама
    #[command(name = "bot")]
    Bot,
    /// Удалить удалённые аккаунты из чата
    #[command(name = "kick-deleted")]
    KickDeleted {
        /// Chat identifier (username or ID) to clean
        #[arg(short, long)]
        chat: Option<String>,
        /// Session file name
        #[arg(short, long, default_value = "kick_deleted_session")]
        session: String,
        /// Run in dry-run mode (don't actually remove users)
        #[arg(long)]
        dry_run: bool,
        /// Pause between operations in seconds
        #[arg(short, long, default_value = "1.0")]
        pause: f64,
    },
}

/// Главная функция: инициализация, загрузка конфигурации и запуск бота
#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Args = Args::parse();

    match args {
        Args::Bot => run_bot().await,
        Args::KickDeleted { chat, session, dry_run, pause } => {
            run_kick_deleted_cli(chat, session, dry_run, pause).await
        }
    }
}

/// Запускает бота для фильтрации спама
async fn run_bot() -> Result<()> {
    let config: Config = Config::from_env()?;
    let whitelist: std::collections::HashSet<i64> = load_whitelist(&config.whitelist_path).await.unwrap_or_else(|e| {
        log::warn!("Не удалось загрузить вайтлист: {}. Используется пустой список.", e);
        std::collections::HashSet::new()
    });
    let state: AppState = AppState::new(whitelist);

    log::info!("Бот запущен. Ожидаю сообщения...");

    // Запускаем задачу удаления удалённых аккаунтов в отдельном таске
    // tokio::spawn(kick_deleted_task_loop());

    run_long_polling(&config, state).await?;

    Ok(())
}

/// Запускает CLI для удаления удалённых аккаунтов
/// Запускает бесконечный цикл задачи удаления удалённых аккаунтов
async fn kick_deleted_task_loop() {
    let interval: Duration = Duration::from_secs(3600); // 1 час
    
    loop {
        if let Err(e) = run_kick_deleted_task().await {
            log::error!("Ошибка при удалении удалённых аккаунтов: {e:?}");
        }
        
        // Подождать перед следующей итерацией
        sleep(interval).await;
    }
}

async fn run_kick_deleted_cli(chat: Option<String>, session: String, dry_run: bool, pause: f64) -> Result<()> {
    let api_id: i32 = std::env::var("TELEGRAM_API_ID")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .ok_or_else(|| anyhow::anyhow!("TELEGRAM_API_ID не задан"))?;

    let api_hash = std::env::var("TELEGRAM_API_HASH")
        .map_err(|_| anyhow::anyhow!("TELEGRAM_API_HASH не задан"))?;

    let phone = std::env::var("TELEGRAM_PHONE")
        .map_err(|_| anyhow::anyhow!("TELEGRAM_PHONE не задан"))?;

    let chat_identifier = chat
        .or_else(|| std::env::var("KICK_DELETED_CHAT").ok())
        .ok_or_else(|| anyhow::anyhow!("Chat identifier не задан (используйте --chat или KICK_DELETED_CHAT)"))?;

    kick_deleted::kick_deleted_users(
        api_id,
        &api_hash,
        &phone,
        &chat_identifier,
        &session,
        dry_run,
        pause,
    ).await?;

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

/// Запускает задачу удаления удалённых аккаунтов
/// Использует переменные окружения для настройки
async fn run_kick_deleted_task() -> Result<()> {
    // Получаем настройки из переменных окружения
    let api_id: i32 = std::env::var("TELEGRAM_API_ID")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or_else(|| {
            log::warn!("TELEGRAM_API_ID не задан, пропускаем задачу удаления");
            return 0;
        });
    
    if api_id == 0 {
        return Ok(());
    }

    let api_hash: String = std::env::var("TELEGRAM_API_HASH")
        .unwrap_or_else(|_| {
            log::warn!("TELEGRAM_API_HASH не задан, пропускаем задачу удаления");
            return String::new();
        });
    
    if api_hash.is_empty() {
        return Ok(());
    }

    let phone = std::env::var("TELEGRAM_PHONE")
        .unwrap_or_else(|_| {
            log::warn!("TELEGRAM_PHONE не задан, пропускаем задачу удаления");
            return String::new();
        });

    if phone.is_empty() {
        return Ok(());
    }

    let chat = std::env::var("KICK_DELETED_CHAT")
        .unwrap_or_else(|_| {
            log::warn!("KICK_DELETED_CHAT не задан, пропускаем задачу удаления");
            return String::new();
        });
    
    if chat.is_empty() {
        return Ok(());
    }

    let session = std::env::var("KICK_DELETED_SESSION")
        .unwrap_or_else(|_| "kick_deleted_session".to_string());

    let dry_run = std::env::var("KICK_DELETED_DRY_RUN")
        .unwrap_or_else(|_| "false".to_string()) == "true";

    let pause: f64 = std::env::var("KICK_DELETED_PAUSE")
        .unwrap_or_else(|_| "1.0".to_string())
        .parse()
        .unwrap_or(1.0);

    log::info!("Запуск задачи удаления удалённых аккаунтов для чата: {}", chat);

    // Call the actual implementation
    crate::kick_deleted::kick_deleted_users(
        api_id,
        &api_hash,
        &phone,
        &chat,
        &session,
        dry_run,
        pause,
    ).await?;

    Ok(())
}
