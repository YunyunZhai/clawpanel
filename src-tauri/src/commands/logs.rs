/// 日志读取命令
/// 使用 BufReader + Seek 避免 OOM，限制最大读取量
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;

/// 日志目录跟随 `openclaw_dir()`，与 service.rs / diagnose.rs 保持一致。
/// 不能写死 `dirs::home_dir()/.openclaw`：便携模式下 openclaw 目录在 U 盘
/// （`data/openclaw`），写死主目录会让日志页永远读到空目录。
fn log_dir() -> PathBuf {
    crate::commands::openclaw_dir().join("logs")
}

/// 已知日志别名 → 实际文件名。保留别名是为了兼容 dashboard / 启动诊断的 key 调用方式。
fn known_log_filename(log_name: &str) -> Option<&'static str> {
    match log_name {
        "gateway" => Some("gateway.log"),
        "gateway-err" => Some("gateway.err.log"),
        "guardian" => Some("guardian.log"),
        "config-audit" => Some("config-audit.jsonl"),
        _ => None,
    }
}

/// 把别名或字面文件名解析成日志目录内的绝对路径。
///
/// 字面文件名来自 `list_log_files` 的扫描结果，但命令是公开的，
/// 因此仍要拒绝分隔符与上级目录跳转。
fn resolve_log_path(log_name: &str) -> Result<PathBuf, String> {
    if let Some(filename) = known_log_filename(log_name) {
        return Ok(log_dir().join(filename));
    }
    if log_name.is_empty()
        || log_name.contains("..")
        || log_name.contains('/')
        || log_name.contains('\\')
    {
        return Err("Invalid log file name".into());
    }
    Ok(log_dir().join(log_name))
}

/// 单个日志文件的元信息，供前端文件列表渲染。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFileInfo {
    name: String,
    size: u64,
    /// Unix 秒；取不到时为 0
    modified: i64,
}

/// 日志文件后缀白名单，与 Hermes 日志页保持一致
fn is_log_file(name: &str) -> bool {
    [".log", ".txt", ".jsonl"]
        .iter()
        .any(|ext| name.ends_with(ext))
}

/// 扫描日志目录，返回真实存在的日志文件（按修改时间倒序）。
///
/// 目录不存在（首次启动还没建）返回空列表而不是报错。
#[tauri::command]
pub fn list_log_files() -> Result<Vec<LogFileInfo>, String> {
    scan_log_dir(&log_dir())
}

fn scan_log_dir(dir: &std::path::Path) -> Result<Vec<LogFileInfo>, String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(format!("读取日志目录失败: {e}")),
    };

    let mut files: Vec<LogFileInfo> = entries
        .flatten()
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_log_file(&name) {
                return None;
            }
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            Some(LogFileInfo {
                name,
                size: meta.len(),
                modified,
            })
        })
        .collect();

    // 修改时间新的在前；同一秒内按文件名排序保证顺序稳定
    files.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(files)
}

#[tauri::command]
pub fn read_log_tail(log_name: String, lines: Option<u32>) -> Result<String, String> {
    let lines = lines.unwrap_or(200) as usize;
    let path = resolve_log_path(&log_name)?;
    if !path.exists() {
        return Ok(String::new());
    }

    let mut file = fs::File::open(&path).map_err(|e| format!("打开日志失败: {e}"))?;

    let file_len = file
        .metadata()
        .map_err(|e| format!("获取文件元数据失败: {e}"))?
        .len();

    // 最多从尾部读取 1MB，避免 OOM
    let max_read: u64 = 1024 * 1024;
    let start_pos = file_len.saturating_sub(max_read);

    file.seek(SeekFrom::Start(start_pos))
        .map_err(|e| format!("Seek 失败: {e}"))?;

    let mut raw = Vec::new();
    file.read_to_end(&mut raw)
        .map_err(|e| format!("读取日志失败: {e}"))?;
    let buf = String::from_utf8_lossy(&raw).into_owned();

    let mut all_lines: Vec<&str> = buf.lines().collect();

    // 如果从中间开始读，第一行可能不完整，跳过
    if start_pos > 0 && all_lines.len() > 1 {
        all_lines.remove(0);
    }

    // 取最后 N 行
    let start = if all_lines.len() > lines {
        all_lines.len() - lines
    } else {
        0
    };

    Ok(all_lines[start..].join("\n"))
}

#[tauri::command]
pub fn search_log(
    log_name: String,
    query: String,
    max_results: Option<u32>,
) -> Result<Vec<String>, String> {
    let max_results = max_results.unwrap_or(50) as usize;
    let path = resolve_log_path(&log_name)?;
    if !path.exists() {
        return Ok(vec![]);
    }

    let mut file = fs::File::open(&path).map_err(|e| format!("打开日志失败: {e}"))?;

    let file_len = file
        .metadata()
        .map_err(|e| format!("获取文件元数据失败: {e}"))?
        .len();

    // 搜索最多读取尾部 2MB，避免 OOM，同时保证搜索最新内容
    let max_read: u64 = 2 * 1024 * 1024;
    let start_pos = file_len.saturating_sub(max_read);

    file.seek(SeekFrom::Start(start_pos))
        .map_err(|e| format!("Seek 失败: {e}"))?;

    let reader = BufReader::new(file);
    let query_lower = query.to_lowercase();

    let mut matched: Vec<String> = reader
        .lines()
        .map_while(Result::ok)
        .filter(|l| l.to_lowercase().contains(&query_lower))
        .collect();

    // 如果从中间开始读，第一条匹配可能是不完整行，跳过
    if start_pos > 0 && !matched.is_empty() {
        matched.remove(0);
    }

    // 取最后 N 条（最新的匹配结果）
    let start = if matched.len() > max_results {
        matched.len() - max_results
    } else {
        0
    };

    Ok(matched[start..].to_vec())
}

#[tauri::command]
pub fn clear_log(log_name: String) -> Result<(), String> {
    let path = resolve_log_path(&log_name)?;
    if !path.exists() {
        return Ok(());
    }
    std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&path)
        .map_err(|e| format!("清除日志失败: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "clawpanel-logs-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn log_dir_follows_openclaw_dir() {
        // 便携模式下 openclaw 目录在 U 盘（data/openclaw），写死 home_dir 会读到空目录
        assert_eq!(log_dir(), crate::commands::openclaw_dir().join("logs"));
    }

    #[test]
    fn known_aliases_still_map_to_filenames() {
        assert_eq!(known_log_filename("gateway"), Some("gateway.log"));
        assert_eq!(known_log_filename("gateway-err"), Some("gateway.err.log"));
        assert_eq!(known_log_filename("guardian"), Some("guardian.log"));
        assert_eq!(
            known_log_filename("config-audit"),
            Some("config-audit.jsonl")
        );
        // 未知 key 走字面文件名分支，不再回退到 gateway.log
        assert_eq!(known_log_filename("gateway-restart.log"), None);
    }

    #[test]
    fn literal_filenames_are_allowed_inside_log_dir() {
        let resolved = resolve_log_path("gateway-restart.log").unwrap();
        assert_eq!(resolved, log_dir().join("gateway-restart.log"));
    }

    #[test]
    fn path_traversal_is_rejected() {
        for name in [
            "",
            "..",
            "../openclaw.json",
            "..\\openclaw.json",
            "sub/dir.log",
            "sub\\dir.log",
        ] {
            assert_eq!(
                resolve_log_path(name),
                Err("Invalid log file name".to_string()),
                "{name} 应该被拒绝"
            );
        }
    }

    #[test]
    fn scan_returns_log_files_sorted_by_modified_desc() {
        let dir = temp_dir("sorted");
        fs::write(dir.join("older.log"), "a\n").unwrap();
        fs::write(dir.join("newer.log"), "b\n").unwrap();
        fs::write(dir.join("audit.jsonl"), "{}\n").unwrap();
        // 非白名单后缀与子目录都不该出现
        fs::write(dir.join("notes.md"), "x\n").unwrap();
        fs::create_dir_all(dir.join("stability")).unwrap();
        fs::write(dir.join("stability").join("event.json"), "{}").unwrap();

        // 显式设置 mtime，避免同秒内排序依赖写入顺序
        let older = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        let newer = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2_000_000);
        set_mtime(&dir.join("older.log"), older);
        set_mtime(&dir.join("audit.jsonl"), older);
        set_mtime(&dir.join("newer.log"), newer);

        let files = scan_log_dir(&dir).unwrap();
        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        // newer.log 最新；older.log 与 audit.jsonl 同一秒，按文件名升序兜底
        assert_eq!(names, vec!["newer.log", "audit.jsonl", "older.log"]);
        assert!(files.iter().all(|f| f.size > 0));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_missing_dir_returns_empty_list() {
        let dir = temp_dir("missing");
        fs::remove_dir_all(&dir).unwrap();
        assert!(scan_log_dir(&dir).unwrap().is_empty());
    }

    #[test]
    fn scan_empty_dir_returns_empty_list() {
        let dir = temp_dir("empty");
        assert!(scan_log_dir(&dir).unwrap().is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    /// std 的 File::set_modified 跨 Windows / Unix 都可用
    fn set_mtime(path: &std::path::Path, time: std::time::SystemTime) {
        let file = fs::OpenOptions::new()
            .write(true)
            .open(path)
            .expect("open for mtime");
        file.set_modified(time).expect("set mtime");
    }
}
