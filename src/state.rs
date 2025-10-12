use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use anyhow::Result;
use tokio::{fs, io::AsyncWriteExt, sync::RwLock};

pub struct AppState {
    pub user_ham_counter: RwLock<HashMap<i64, u32>>,
    pub whitelist_cache: RwLock<HashSet<i64>>,
}

impl AppState {
    /// Создаёт новое состояние приложения с переданным набором пользователей в вайтлисте.
    /// Используется для хранения счётчиков HAM и кэша вайтлиста.
    pub fn new(whitelist: HashSet<i64>) -> Self {
        Self {
            user_ham_counter: RwLock::new(HashMap::new()),
            whitelist_cache: RwLock::new(whitelist),
        }
    }
}

/// Проверяет, находится ли пользователь в кэшированном вайтлисте.
pub async fn is_user_whitelisted(user_id: i64, state: &AppState) -> Result<bool> {
    let cache: tokio::sync::RwLockReadGuard<'_, HashSet<i64>> = state.whitelist_cache.read().await;
    Ok(cache.contains(&user_id))
}

/// Добавляет пользователя в вайтлист (кэш и файл).
pub async fn add_user_to_whitelist(user_id: i64, state: &AppState, whitelist_path: &PathBuf) -> Result<()> {
    {
        let mut cache: tokio::sync::RwLockWriteGuard<'_, HashSet<i64>> = state.whitelist_cache.write().await;
        if !cache.insert(user_id) {
            return Ok(());
        }
    }
    append_line_to_file(whitelist_path, &user_id.to_string()).await
}

/// Увеличивает счётчик не-СПАМ сообщений для пользователя и возвращает текущее значение.
pub async fn increment_ham_counter(user_id: i64, state: &AppState) -> u32 {
    let mut map: tokio::sync::RwLockWriteGuard<'_, HashMap<i64, u32>> = state.user_ham_counter.write().await;
    let entry: &mut u32 = map.entry(user_id).or_insert(0);
    *entry += 1;
    *entry
}

/// Загружает вайтлист из файла, возвращая пустой если файл не существует.
/// Формат файла: по одному user_id на строку.
pub async fn load_whitelist(path: &PathBuf) -> Result<HashSet<i64>> {
    if !path.exists() {
        return Ok(HashSet::new());
    }

    let content: String = fs::read_to_string(path).await.unwrap_or_else(|e| {
        log::warn!("Не удалось прочитать файл вайтлиста {}: {}", path.display(), e);
        String::new()
    });
    
    Ok(content
        .lines()
        .filter_map(|l| l.trim().parse::<i64>().ok())
        .collect())
}

/// Добавляет строку в конец файла, создавая его при необходимости.
/// Вспомогательная функция, используется только внутри модуля.
async fn append_line_to_file(path: &PathBuf, line: &str) -> Result<()> {
    let mut file: fs::File = if path.exists() {
        tokio::fs::OpenOptions::new().append(true).open(path).await?
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.ok();
        }
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?
    };
    file.write_all(format!("{}\n", line).as_bytes()).await?;
    Ok(())
}
