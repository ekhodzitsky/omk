#![allow(dead_code)]

use super::parser::Skill;

/// Build a prompt with skill injection
pub fn inject_skill(skill: &Skill, user_prompt: &str) -> String {
    format!(
        "{skill_header}\n{skill_body}\n\n---\n\n{user_prompt}",
        skill_header = format!("### Skill: {}\n{}", skill.name, skill.description),
        skill_body = skill.body,
        user_prompt = user_prompt,
    )
}

/// Check if prompt matches any skill trigger
pub fn match_trigger<'a>(skills: &'a [Skill], prompt: &str) -> Option<&'a Skill> {
    let prompt_lower = prompt.to_lowercase();
    skills.iter().find(|s| {
        s.triggers
            .iter()
            .any(|t| prompt_lower.contains(&t.to_lowercase()))
    })
}
