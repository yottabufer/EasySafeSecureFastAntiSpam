use std::{
    collections::HashSet,
    path::PathBuf,
};

use anyhow::Result;
use tokio::fs;

/// Загружает вайтлист из файла, возвращая пустой если файл не существует.
pub async fn load_whitelist(path: &PathBuf) -> Result<HashSet<i64>> {
    if !path.exists() {
        return Ok(HashSet::new());
    }

    let content = match fs::read_to_string(path).await {
        Ok(content) => content,
        Err(e) => {
            log::warn!("Не удалось прочитать файл вайтлиста {}: {}", path.display(), e);
            return Ok(HashSet::new());
        }
    };
    let set = content
        .lines()
        .filter_map(|l| l.trim().parse::<i64>().ok())
        .collect::<HashSet<_>>();
    Ok(set)
}

/// Добавляет строку в конец файла, создавая его при необходимости.
pub async fn append_line(path: &PathBuf, line: &str) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut file = if path.exists() {
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .await?
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
