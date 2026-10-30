use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quiz {
    pub topic: String,
    #[serde(default)]
    pub source_note: String,
    pub questions: Vec<Question>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub question: String,
    pub answer: String,
    #[serde(default)]
    pub source_page: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub key_points: Vec<KeyPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPoint {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub wrong_claims: Vec<String>,
}

impl Quiz {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Quiz> {
        Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
    }

    /// Every `*.quiz.json` in `dir`, sorted by file name. Unreadable files are skipped.
    pub fn load_dir(dir: &std::path::Path) -> Vec<Quiz> {
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.to_string_lossy().ends_with(".quiz.json"))
            .collect();
        paths.sort();
        paths.iter().filter_map(|p| Quiz::load(p).ok()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_existing_datasets() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        for name in ["rate-limiter.quiz.json", "sdi-ch4-8.quiz.json"] {
            let q = Quiz::load(&root.join(name)).unwrap();
            assert!(!q.questions.is_empty());
            assert!(q.questions.iter().all(|q| !q.key_points.is_empty()));
        }
    }
}
