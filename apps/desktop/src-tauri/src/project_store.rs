use std::fs;
use std::path::Path;

use videocut_model::Project;

pub fn read_project_json(path: &str) -> Result<String, String> {
    let text =
        fs::read_to_string(Path::new(path)).map_err(|error| format!("読めません: {error}"))?;
    let project: Project = serde_json::from_str(&text)
        .map_err(|error| format!("プロジェクトの形式が違います: {error}"))?;
    serde_json::to_string_pretty(&project).map_err(|error| error.to_string())
}

pub fn write_project_json(path: &str, project_json: &str) -> Result<(), String> {
    let project: Project = serde_json::from_str(project_json)
        .map_err(|error| format!("プロジェクトの形式が違います: {error}"))?;
    let text = serde_json::to_string_pretty(&project).map_err(|error| error.to_string())?;
    fs::write(Path::new(path), format!("{text}\n")).map_err(|error| format!("書けません: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_vcut_matches_the_model() {
        let dir = std::env::temp_dir().join(format!("videocut-shell-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("demo.vcut");
        let original = r#"{"id":"proj-1","name":"demo","bins":[],"sequences":[],"media":[]}"#;
        write_project_json(path.to_str().unwrap(), original).unwrap();
        let loaded = read_project_json(path.to_str().unwrap()).unwrap();
        let project: Project = serde_json::from_str(&loaded).unwrap();
        assert_eq!(project.name, "demo");
        assert_eq!(project.id, "proj-1");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_a_file_that_is_not_a_project() {
        let dir = std::env::temp_dir().join(format!("videocut-shell-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.vcut");
        fs::write(&path, "{}\n").unwrap();
        assert!(read_project_json(path.to_str().unwrap()).is_err());
        assert!(write_project_json(path.to_str().unwrap(), "{\"name\":1}").is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
