use serde::{Deserialize, Serialize};

// ============================================================================
// ContentPart
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text(TextPart),
    Think(ThinkPart),
    #[serde(rename = "image_url")]
    ImageUrl(ImageUrlPart),
    #[serde(rename = "audio_url")]
    AudioUrl(AudioUrlPart),
    #[serde(rename = "video_url")]
    VideoUrl(VideoUrlPart),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextPart {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThinkPart {
    pub think: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageUrlPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioUrlPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoUrlPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

// ============================================================================
// DisplayBlock
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DisplayBlock {
    Brief(BriefDisplayBlock),
    Diff(DiffDisplayBlock),
    Todo(TodoDisplayBlock),
    Shell(ShellDisplayBlock),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BriefDisplayBlock {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiffDisplayBlock {
    pub path: String,
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TodoDisplayBlock {
    pub items: Vec<TodoDisplayItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TodoDisplayItem {
    pub title: String,
    pub status: TodoStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShellDisplayBlock {
    pub language: String,
    pub command: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_part_image_url() {
        let part = ContentPart::ImageUrl(ImageUrlPart {
            url: Some("https://example.com/img.png".to_string()),
        });
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json["type"], "image_url");
        assert_eq!(json["url"], "https://example.com/img.png");
        let de: ContentPart = serde_json::from_value(json).unwrap();
        assert_eq!(de, part);
    }

    #[test]
    fn test_content_part_audio_url() {
        let part = ContentPart::AudioUrl(AudioUrlPart {
            url: Some("https://example.com/audio.mp3".to_string()),
        });
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json["type"], "audio_url");
        let de: ContentPart = serde_json::from_value(json).unwrap();
        assert_eq!(de, part);
    }

    #[test]
    fn test_content_part_video_url() {
        let part = ContentPart::VideoUrl(VideoUrlPart {
            url: Some("https://example.com/video.mp4".to_string()),
        });
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json["type"], "video_url");
        let de: ContentPart = serde_json::from_value(json).unwrap();
        assert_eq!(de, part);
    }

    #[test]
    fn test_display_block_brief() {
        let block = DisplayBlock::Brief(BriefDisplayBlock {
            text: "summary".to_string(),
        });
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "brief");
        assert_eq!(json["text"], "summary");
        let de: DisplayBlock = serde_json::from_value(json).unwrap();
        assert_eq!(de, block);
    }

    #[test]
    fn test_display_block_diff() {
        let block = DisplayBlock::Diff(DiffDisplayBlock {
            path: "/tmp/test.txt".to_string(),
            old_text: "old".to_string(),
            new_text: "new".to_string(),
        });
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "diff");
        assert_eq!(json["path"], "/tmp/test.txt");
        let de: DisplayBlock = serde_json::from_value(json).unwrap();
        assert_eq!(de, block);
    }

    #[test]
    fn test_display_block_todo() {
        let block = DisplayBlock::Todo(TodoDisplayBlock {
            items: vec![
                TodoDisplayItem {
                    title: "task 1".to_string(),
                    status: TodoStatus::Pending,
                },
                TodoDisplayItem {
                    title: "task 2".to_string(),
                    status: TodoStatus::Done,
                },
            ],
        });
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "todo");
        assert_eq!(json["items"][0]["status"], "pending");
        assert_eq!(json["items"][1]["status"], "done");
        let de: DisplayBlock = serde_json::from_value(json).unwrap();
        assert_eq!(de, block);
    }

    #[test]
    fn test_display_block_shell() {
        let block = DisplayBlock::Shell(ShellDisplayBlock {
            language: "bash".to_string(),
            command: "echo hello".to_string(),
        });
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "shell");
        assert_eq!(json["language"], "bash");
        assert_eq!(json["command"], "echo hello");
        let de: DisplayBlock = serde_json::from_value(json).unwrap();
        assert_eq!(de, block);
    }
}
