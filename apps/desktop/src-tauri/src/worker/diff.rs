use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedDiffResult {
    pub file_path: String,
    pub is_new: bool,
    pub additions: usize,
    pub deletions: usize,
}

/// Parses SEARCH/REPLACE or CREATE diff blocks from model response and applies them directly to worktree files.
pub fn apply_diff_blocks(
    worktree_dir: &Path,
    response_text: &str,
    fallback_target_file: Option<&str>,
) -> Result<Vec<AppliedDiffResult>, String> {
    let mut results = Vec::new();
    let lines: Vec<&str> = response_text.lines().collect();
    let mut i = 0;
    let mut current_file = fallback_target_file.map(|s| s.to_string());

    while i < lines.len() {
        let line = lines[i].trim();

        // Check for file indicator: "FILE: path/to/file.ext" or "### File: path/to/file.ext"
        if let Some(rest) = line.strip_prefix("FILE:").or_else(|| line.strip_prefix("File:")) {
            current_file = Some(rest.trim().trim_matches('`').trim().to_string());
            i += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix("### File:") {
            current_file = Some(rest.trim().trim_matches('`').trim().to_string());
            i += 1;
            continue;
        }

        // Check for CREATE block: "<<<<<<< CREATE: path/to/file.ext"
        if line.starts_with("<<<<<<< CREATE") {
            let file_target = if let Some(pos) = line.find(':') {
                line[pos + 1..].trim().trim_matches('`').to_string()
            } else {
                current_file.clone().ok_or_else(|| "CREATE block missing target file path".to_string())?
            };

            i += 1;
            let mut create_lines = Vec::new();
            while i < lines.len() && !lines[i].trim().starts_with(">>>>>>>") {
                create_lines.push(lines[i]);
                i += 1;
            }
            if i < lines.len() && lines[i].trim().starts_with(">>>>>>>") {
                i += 1; // skip >>>>>>>
            }

            let full_target = worktree_dir.join(&file_target);
            if let Some(parent) = full_target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let new_content = create_lines.join("\n");
            let line_count = create_lines.len();
            std::fs::write(&full_target, &new_content)
                .map_err(|e| format!("Failed to write new file {file_target}: {e}"))?;

            results.push(AppliedDiffResult {
                file_path: file_target,
                is_new: true,
                additions: line_count,
                deletions: 0,
            });
            continue;
        }

        // Check for SEARCH/REPLACE block
        if line.starts_with("<<<<<<< SEARCH") {
            // Can optionally have "<<<<<<< SEARCH: path/to/file.ext"
            if let Some(pos) = line.find(':') {
                current_file = Some(line[pos + 1..].trim().trim_matches('`').to_string());
            }

            let file_target = current_file.clone().ok_or_else(|| {
                "SEARCH block found without an active target file path".to_string()
            })?;

            i += 1;
            let mut search_lines = Vec::new();
            while i < lines.len() && !lines[i].trim().starts_with("=======") {
                search_lines.push(lines[i]);
                i += 1;
            }

            if i >= lines.len() {
                return Err(format!("Unclosed SEARCH block in file {file_target}"));
            }

            i += 1; // skip =======

            let mut replace_lines = Vec::new();
            while i < lines.len() && !lines[i].trim().starts_with(">>>>>>>") {
                replace_lines.push(lines[i]);
                i += 1;
            }

            if i < lines.len() && lines[i].trim().starts_with(">>>>>>>") {
                i += 1; // skip >>>>>>>
            }

            let full_target = worktree_dir.join(&file_target);
            if !full_target.exists() {
                return Err(format!("Target file for SEARCH/REPLACE does not exist: {file_target}"));
            }

            let existing_content = std::fs::read_to_string(&full_target)
                .map_err(|e| format!("Failed to read target file {file_target}: {e}"))?;

            let search_str = search_lines.join("\n");
            let replace_str = replace_lines.join("\n");

            // Normalize line endings for reliable matching on Windows
            let norm_existing = existing_content.replace("\r\n", "\n");
            let norm_search = search_str.replace("\r\n", "\n");
            let norm_replace = replace_str.replace("\r\n", "\n");

            let updated_content = if let Some(match_idx) = norm_existing.find(&norm_search) {
                let mut updated = norm_existing[..match_idx].to_string();
                updated.push_str(&norm_replace);
                updated.push_str(&norm_existing[match_idx + norm_search.len()..]);
                updated
            } else {
                // Fallback: line-trimmed match
                let existing_lines: Vec<&str> = norm_existing.lines().collect();
                let s_lines: Vec<&str> = norm_search.lines().collect();
                if let Some(found_idx) = find_subslice_trimmed(&existing_lines, &s_lines) {
                    let mut updated_lines = Vec::new();
                    for idx in 0..found_idx {
                        updated_lines.push(existing_lines[idx]);
                    }
                    for r_line in norm_replace.lines() {
                        updated_lines.push(r_line);
                    }
                    for idx in (found_idx + s_lines.len())..existing_lines.len() {
                        updated_lines.push(existing_lines[idx]);
                    }
                    updated_lines.join("\n")
                } else {
                    return Err(format!(
                        "Could not locate SEARCH block in {file_target}.\nSEARCH snippet was:\n{search_str}"
                    ));
                }
            };

            std::fs::write(&full_target, &updated_content)
                .map_err(|e| format!("Failed to write updated file {file_target}: {e}"))?;

            let additions = replace_lines.len();
            let deletions = search_lines.len();

            results.push(AppliedDiffResult {
                file_path: file_target,
                is_new: false,
                additions,
                deletions,
            });
            continue;
        }

        i += 1;
    }

    Ok(results)
}

fn find_subslice_trimmed(haystack: &[&str], needle: &[&str]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    'outer: for i in 0..=(haystack.len() - needle.len()) {
        for j in 0..needle.len() {
            if haystack[i + j].trim() != needle[j].trim() {
                continue 'outer;
            }
        }
        return Some(i);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_diff_blocks_replace_and_create() {
        let temp = std::env::temp_dir().join(format!("test_diff_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
        std::fs::create_dir_all(&temp).unwrap();

        let initial_calc = "def add(a, b):\n    return a + b\n\ndef sub(a, b):\n    return a - b\n";
        std::fs::write(temp.join("calc.py"), initial_calc).unwrap();

        let diff_response = r#"I will add the multiply function to calc.py and create test_calc.py.

FILE: calc.py
<<<<<<< SEARCH
def sub(a, b):
    return a - b
=======
def sub(a, b):
    return a - b

def multiply(a, b):
    return a * b
>>>>>>> REPLACE

<<<<<<< CREATE: test_calc.py
from calc import add, multiply

def test():
    assert multiply(2, 3) == 6
>>>>>>>
"#;

        let results = apply_diff_blocks(&temp, diff_response, None).unwrap();
        assert_eq!(results.len(), 2);

        let final_calc = std::fs::read_to_string(temp.join("calc.py")).unwrap();
        assert!(final_calc.contains("def multiply(a, b):"));
        assert!(final_calc.contains("def add(a, b):"));

        let final_test = std::fs::read_to_string(temp.join("test_calc.py")).unwrap();
        assert!(final_test.contains("assert multiply(2, 3) == 6"));

        let _ = std::fs::remove_dir_all(&temp);
    }
}
