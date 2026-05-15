// Ralph PRD generation — break a task into user stories.
use anyhow::Result;
use regex::Regex;

use crate::runtime::state::{Prd, StoryStatus, UserStory};

pub(crate) fn slugify_task(task: &str) -> Result<String> {
    let word_re = Regex::new(r"\b\w+\b")?;
    let words: Vec<&str> = word_re.find_iter(task).map(|m| m.as_str()).collect();
    let slug = words[..words.len().min(5)].join("-").to_lowercase();
    if slug.is_empty() {
        Ok("untitled".to_string())
    } else {
        Ok(slug)
    }
}

/// Generate a PRD by breaking a task into 3–5 user stories.
pub fn generate_prd(task: &str) -> Prd {
    let sentences: Vec<&str> = task
        .split(['.', '?', '!'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let chunks: Vec<String> = if sentences.len() >= 3 {
        sentences.into_iter().map(String::from).collect()
    } else {
        task.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    let stories: Vec<UserStory> = chunks
        .into_iter()
        .enumerate()
        .map(|(i, desc)| UserStory {
            id: format!("US-{:03}", i + 1),
            description: desc.clone(),
            acceptance_criteria: vec![
                format!("{} is implemented correctly", desc),
                "All related tests pass".to_string(),
            ],
            status: StoryStatus::NotStarted,
        })
        .take(5)
        .collect();

    if stories.is_empty() {
        Prd {
            user_stories: vec![UserStory {
                id: "US-001".to_string(),
                description: task.to_string(),
                acceptance_criteria: vec![
                    format!("{} is implemented correctly", task),
                    "All related tests pass".to_string(),
                ],
                status: StoryStatus::NotStarted,
            }],
        }
    } else {
        Prd {
            user_stories: stories,
        }
    }
}
