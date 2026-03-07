use std::path::PathBuf;
use std::fs;
use anyhow::Result;
use directories::ProjectDirs;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "firescan";
const APPLICATION: &str = "firescan";

pub fn get_app_dirs() -> Result<ProjectDirs> {
    directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
        .ok_or_else(|| anyhow::anyhow!("Failed to determine app directories"))
}

pub fn get_library_path() -> Result<PathBuf> {
    let dirs = get_app_dirs()?;
    let default = if cfg!(target_os = "windows") {
        PathBuf::from(r"C:\mangás")
    } else {
        dirs.data_local_dir().join("library")
    };

    // Allow override via a small file `library_path.txt` in the data dir
    let override_file = dirs.data_local_dir().join("library_path.txt");
    if override_file.exists() {
        if let Ok(contents) = std::fs::read_to_string(&override_file) {
            let s = contents.lines().next().unwrap_or("").trim();
            if !s.is_empty() {
                let p = PathBuf::from(s);
                std::fs::create_dir_all(&p)?;
                return Ok(p);
            }
        }
    }

    std::fs::create_dir_all(&default)?;

    Ok(default)
}

pub fn set_library_path(path: &PathBuf) -> Result<()> {
    let dirs = get_app_dirs()?;
    std::fs::create_dir_all(path)?;
    let override_file = dirs.data_local_dir().join("library_path.txt");
    std::fs::create_dir_all(dirs.data_local_dir())?;
    std::fs::write(&override_file, path.to_string_lossy().to_string())?;
    Ok(())
}

pub fn get_cache_path() -> Result<PathBuf> {
    let dirs = get_app_dirs()?;
    Ok(dirs.cache_dir().to_path_buf())
}

pub fn get_database_path() -> Result<PathBuf> {
    let dirs = get_app_dirs()?;
    Ok(dirs.data_local_dir().join("app.db"))
}

pub fn get_logs_path() -> Result<PathBuf> {
    let dirs = get_app_dirs()?;
    Ok(dirs.data_local_dir().join("logs"))
}

pub fn initialize_app_paths() -> Result<()> {
    // Create all necessary directories
    fs::create_dir_all(get_library_path()?)?;
    fs::create_dir_all(get_cache_path()?)?;
    fs::create_dir_all(get_logs_path()?)?;
    
    Ok(())
}
