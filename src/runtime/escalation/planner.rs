pub fn plan_medium(prompt: &str) -> anyhow::Result<Vec<String>> {
    let delimiters = [" and ", " + ", "; ", "\n", ". "];

    let mut parts = vec![prompt.to_string()];
    for delim in delimiters {
        let mut new_parts = Vec::new();
        for part in &parts {
            for sub in part.split(delim) {
                let trimmed = sub.trim();
                if !trimmed.is_empty() {
                    new_parts.push(trimmed.to_string());
                }
            }
        }
        parts = new_parts;
    }

    let mut steps: Vec<String> = Vec::new();
    for part in parts {
        if part.len() < 10 {
            if let Some(last) = steps.last_mut() {
                last.push(' ');
                last.push_str(&part);
                continue;
            }
        }
        steps.push(part);
    }

    if steps.len() < 3 {
        Ok(vec![prompt.to_string()])
    } else {
        steps.truncate(7);
        Ok(steps)
    }
}
