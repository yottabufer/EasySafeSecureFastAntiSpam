use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use anyhow::Result;
use tokio::sync::RwLock;

pub struct AppState {
    pub user_ham_counter: RwLock<HashMap<i64, u32>>,
    pub whitelist_cache: RwLock<HashSet<i64>>,
}

impl AppState {
    /// Создаёт новое состояние приложения с переданным набором пользователей в вайтлисте.
    pub fn new(whitelist: HashSet<i64>) -> Self {
        Self {
            user_ham_counter: RwLock::new(HashMap::new()),
            whitelist_cache: RwLock::new(whitelist),
        }
    }
}

/// Проверяет, находится ли пользователь в кэшированном вайтлисте.
pub async fn is_user_whitelisted(user_id: i64, state: &AppState) -> Result<bool> {
    let cache = state.whitelist_cache.read().await;
    Ok(cache.contains(&user_id))
}

/// Добавляет пользователя в вайтлист (кэш и файл).
pub async fn add_user_to_whitelist(user_id: i64, state: &AppState, whitelist_path: &PathBuf) -> Result<()> {
    {
        let mut cache = state.whitelist_cache.write().await;
        if !cache.insert(user_id) {
            return Ok(());
        }
    }
    crate::white_list::append_line(whitelist_path, &user_id.to_string()).await
}

/// Увеличивает счётчик не-СПАМ сообщений для пользователя и возвращает текущее значение.
pub async fn increment_ham_counter(user_id: i64, state: &AppState) -> u32 {
    let mut map = state.user_ham_counter.write().await;
    let entry = map.entry(user_id).or_insert(0);
    *entry += 1;
    *entry
}
