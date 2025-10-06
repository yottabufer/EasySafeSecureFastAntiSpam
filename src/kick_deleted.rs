use anyhow::Result;
use grammers_client::{Client, Config, SignInError};
use grammers_session::Session;
use std::time::Duration;
use tokio::time::sleep;
use std::io::{self, Write};

pub async fn kick_deleted_users(
    api_id: i32,
    api_hash: &str,
    phone: &str,
    chat_identifier: &str,
    session_file: &str,
    dry_run: bool,
    pause: f64,
) -> Result<()> {
    log::info!(
        "Запуск задачи удаления удалённых аккаунтов для чата: {}, dry_run: {}",
        chat_identifier,
        dry_run
    );

    let config: Config = Config {
        session: Session::load_file_or_create(session_file)?,
        api_id,
        api_hash: api_hash.to_string(),
        params: Default::default(),
    };

    let mut client: Client = Client::connect(config).await?;
    
    if !client.is_authorized().await? {
        log::info!("Требуется авторизация. Отправляем код на номер: {}", phone);
        
        let token: grammers_client::types::LoginToken = client.request_login_code(phone).await?;
        print!("Введите код подтверждения, отправленный на {}: ", phone);
        io::stdout().flush()?; // Ensure the prompt is displayed
        
        let mut code: String = String::new();
        io::stdin().read_line(&mut code)?;
        let code: &str = code.trim();
        
        match client.sign_in(&token, code).await {
            Ok(_) => {
                log::info!("Успешная авторизация");
                client.session().save_to_file(session_file)?;
            },
            Err(SignInError::PasswordRequired(_)) => {
                anyhow::bail!("Требуется двухфакторная аутентификация, которая не поддерживается в этом примере");
            }
            Err(e) => anyhow::bail!("Ошибка авторизации: {:?}", e),
        }
    }

    let chat: grammers_client::types::Chat = find_chat(&client, chat_identifier).await?;
    
    log::info!("Подключено к чату: {}", 
        chat.username().unwrap_or_else(|| chat.name())
    );
    
    let mut participants: grammers_client::client::chats::ParticipantIter = client.iter_participants(&chat);
    
    let mut deleted_users: Vec<grammers_client::types::Participant> = Vec::new();
    
    log::info!("Сканируем участников чата...");
    
    while let Some(user) = participants.next().await? {
        let is_deleted: bool = user.user.first_name() == "Deleted Account" || 
                       user.user.first_name().contains("Deleted") ||
                       user.user.username().is_none() && user.user.phone().is_none() && 
                           (user.user.first_name().is_empty() || user.user.first_name() == "");
        
        if is_deleted {
            log::info!("Найден удалённый аккаунт: {} (ID: {})", 
                user.user.username().unwrap_or("<без username>"), 
                user.user.id()
            );
            deleted_users.push(user);
        }
    }
    
    log::info!("Найдено {} удалённых аккаунтов", deleted_users.len());
    
    if deleted_users.is_empty() {
        log::info!("Не найдено удалённых аккаунтов для удаления.");
        return Ok(());
    }

    for user in deleted_users {
        if dry_run {
            log::info!("[DRY RUN] Удаление пользователя: {} (ID: {})", 
                user.user.username().unwrap_or("<без username>"), 
                user.user.id()
            );
        } else {
            log::info!("Удаляем пользователя: {} (ID: {})", 
                user.user.username().unwrap_or("<без username>"), 
                user.user.id()
            );
            
            match client.kick_participant(&chat, &user.user).await {
                Ok(_) => {
                    log::info!("Успешно удалён: {} (ID: {})", 
                        user.user.username().unwrap_or("<без username>"), 
                        user.user.id()
                    );
                }
                Err(e) => {
                    log::error!("Ошибка при удалении {}: {}", 
                        user.user.username().unwrap_or("<deleted user>"), 
                        e
                    );
                }
            }
            
            sleep(Duration::from_millis((pause * 1000.0) as u64)).await;
        }
    }
    
    log::info!("Задача удаления удалённых аккаунтов завершена");

    Ok(())
}

async fn find_chat(client: &Client, chat_identifier: &str) -> Result<grammers_client::types::Chat> {
    if chat_identifier.starts_with('@') {
        let username: &str = &chat_identifier[1..];
        match client.resolve_username(username).await {
            Ok(Some(chat)) => Ok(chat),
            Ok(None) => anyhow::bail!("Чат с username '{}' не найден", username),
            Err(e) => anyhow::bail!("Ошибка при поиске чата по username '{}': {}", username, e),
        }
    } else {
        match chat_identifier.parse::<i64>() {
            Ok(chat_id) => {
                let mut dialogs: grammers_client::types::IterBuffer<grammers_client::grammers_tl_types::functions::messages::GetDialogs, grammers_client::types::Dialog> = client.iter_dialogs();
                while let Some(dialog) = dialogs.next().await? {
                    let chat: &grammers_client::types::Chat = dialog.chat();
                    if chat.id() == chat_id {
                        return Ok(chat.clone());
                    }
                }
                anyhow::bail!("Чат с ID {} не найден в ваших диалогах", chat_id)
            }
            Err(_) => anyhow::bail!("Неверный формат идентификатора чата: '{}'", chat_identifier),
        }
    }
}