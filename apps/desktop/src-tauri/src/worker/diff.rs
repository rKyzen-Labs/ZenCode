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

        // Check for DELETE block: "<<<<<<< DELETE: path/to/file.ext" or "DELETE: path/to/file.ext"
        if line.starts_with("<<<<<<< DELETE") || line.starts_with("DELETE:") || line.starts_with("REMOVE:") {
            let file_target = if let Some(pos) = line.find(':') {
                let rest = &line[pos + 1..];
                let end = rest.find(">>>>>>>").unwrap_or(rest.len());
                rest[..end].trim().trim_matches('`').to_string()
            } else {
                current_file.clone().ok_or_else(|| "DELETE block missing target file path".to_string())?
            };

            if line.starts_with("<<<<<<<") && !line.contains(">>>>>>>") {
                i += 1;
                while i < lines.len() && !lines[i].trim().starts_with(">>>>>>>") {
                    i += 1;
                }
                if i < lines.len() && lines[i].trim().starts_with(">>>>>>>") {
                    i += 1;
                }
            } else {
                i += 1;
            }

            let full_target = worktree_dir.join(&file_target);
            let line_count = if full_target.exists() {
                std::fs::read_to_string(&full_target).map(|s| s.lines().count()).unwrap_or(0)
            } else {
                0
            };
            if full_target.exists() {
                let _ = std::fs::remove_file(&full_target);
            }

            results.push(AppliedDiffResult {
                file_path: file_target,
                is_new: false,
                additions: 0,
                deletions: line_count,
            });
            continue;
        }

        // Check for MKDIR block: "<<<<<<< MKDIR: path/to/folder" or "MKDIR: path/to/folder"
        if line.starts_with("<<<<<<< MKDIR") || line.starts_with("MKDIR:") || line.starts_with("CREATE_DIR:") {
            let folder_target = if let Some(pos) = line.find(':') {
                let rest = &line[pos + 1..];
                let end = rest.find(">>>>>>>").unwrap_or(rest.len());
                rest[..end].trim().trim_matches('`').to_string()
            } else {
                i += 1;
                continue;
            };

            if line.starts_with("<<<<<<<") && !line.contains(">>>>>>>") {
                i += 1;
                while i < lines.len() && !lines[i].trim().starts_with(">>>>>>>") {
                    i += 1;
                }
                if i < lines.len() && lines[i].trim().starts_with(">>>>>>>") {
                    i += 1;
                }
            } else {
                i += 1;
            }

            let full_target = worktree_dir.join(&folder_target);
            let _ = std::fs::create_dir_all(&full_target);

            results.push(AppliedDiffResult {
                file_path: folder_target,
                is_new: true,
                additions: 0,
                deletions: 0,
            });
            continue;
        }

        // Check for MOVE / RENAME block: "<<<<<<< MOVE: src -> dst" or "MOVE: src -> dst"
        if line.starts_with("<<<<<<< MOVE") || line.starts_with("MOVE:") || line.starts_with("RENAME:") || line.starts_with("<<<<<<< RENAME") {
            let spec = if let Some(pos) = line.find(':') {
                let rest = &line[pos + 1..];
                let end = rest.find(">>>>>>>").unwrap_or(rest.len());
                rest[..end].trim().to_string()
            } else {
                i += 1;
                continue;
            };

            if line.starts_with("<<<<<<<") && !line.contains(">>>>>>>") {
                i += 1;
                while i < lines.len() && !lines[i].trim().starts_with(">>>>>>>") {
                    i += 1;
                }
                if i < lines.len() && lines[i].trim().starts_with(">>>>>>>") {
                    i += 1;
                }
            } else {
                i += 1;
            }

            if let Some((src, dst)) = spec.split_once("->") {
                let src_clean = src.trim().trim_matches('`').trim();
                let dst_clean = dst.trim().trim_matches('`').trim();

                let full_src = worktree_dir.join(src_clean);
                let full_dst = worktree_dir.join(dst_clean);

                if let Some(parent) = full_dst.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }

                if full_src.exists() {
                    let _ = std::fs::rename(&full_src, &full_dst);
                }

                results.push(AppliedDiffResult {
                    file_path: format!("{src_clean} -> {dst_clean}"),
                    is_new: false,
                    additions: 1,
                    deletions: 1,
                });
            }
            continue;
        }

        // Check for RMDIR block: "<<<<<<< RMDIR: path/to/folder" or "RMDIR: path/to/folder"
        if line.starts_with("<<<<<<< RMDIR") || line.starts_with("RMDIR:") || line.starts_with("DELETE_DIR:") {
            let folder_target = if let Some(pos) = line.find(':') {
                let rest = &line[pos + 1..];
                let end = rest.find(">>>>>>>").unwrap_or(rest.len());
                rest[..end].trim().trim_matches('`').to_string()
            } else {
                i += 1;
                continue;
            };

            if line.starts_with("<<<<<<<") && !line.contains(">>>>>>>") {
                i += 1;
                while i < lines.len() && !lines[i].trim().starts_with(">>>>>>>") {
                    i += 1;
                }
                if i < lines.len() && lines[i].trim().starts_with(">>>>>>>") {
                    i += 1;
                }
            } else {
                i += 1;
            }

            let full_target = worktree_dir.join(&folder_target);
            if full_target.exists() {
                let _ = std::fs::remove_dir_all(&full_target);
            }

            results.push(AppliedDiffResult {
                file_path: folder_target,
                is_new: false,
                additions: 0,
                deletions: 1,
            });
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
                // Fallback 1: line-trimmed match
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
                } else if let Some(found_idx) = find_fuzzy_comment_subslice(&existing_lines, &s_lines) {
                    // Fallback 2: Comment/punctuation normalized fuzzy match
                    // Handles differences where the model emitted `/ comment /` instead of `/* comment */`
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
                } else if let Some((start_idx, end_idx)) = find_anchor_subslice(&existing_lines, &s_lines) {
                    // Fallback 3: First-and-last line anchor match
                    let mut updated_lines = Vec::new();
                    for idx in 0..start_idx {
                        updated_lines.push(existing_lines[idx]);
                    }
                    for r_line in norm_replace.lines() {
                        updated_lines.push(r_line);
                    }
                    for idx in end_idx..existing_lines.len() {
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

    // Fallback: If no diff blocks were matched (e.g. model outputted standard markdown code blocks),
    // parse markdown fenced blocks with associated file headers/comments.
    if results.is_empty() {
        let markdown_results = apply_markdown_file_blocks(worktree_dir, response_text, fallback_target_file)?;
        results.extend(markdown_results);
    }

    Ok(results)
}

/// Fallback parser for standard LLM markdown responses with code fences.
/// Supports `### File: path`, `FILE: path`, `**path**`, ````lang:path````, and in-fence file comments.
pub fn apply_markdown_file_blocks(
    worktree_dir: &Path,
    response_text: &str,
    fallback_target_file: Option<&str>,
) -> Result<Vec<AppliedDiffResult>, String> {
    let mut results = Vec::new();
    let lines: Vec<&str> = response_text.lines().collect();
    let mut i = 0;
    let mut pending_file_header: Option<String> = None;
    let mut collected_blocks: Vec<(Option<String>, Vec<String>)> = Vec::new();

    while i < lines.len() {
        let line = lines[i].trim();

        // 1. Detect markdown file headers preceding code fences
        if let Some(rest) = line.strip_prefix("FILE:").or_else(|| line.strip_prefix("File:")).or_else(|| line.strip_prefix("Filename:")) {
            let clean = rest.trim().trim_matches('`').trim_matches('*').trim();
            if is_valid_filename(clean) {
                pending_file_header = Some(clean.to_string());
            }
            i += 1;
            continue;
        }

        if line.starts_with("###") || line.starts_with("##") {
            let header_text = line.trim_start_matches('#').trim();
            let clean = header_text
                .trim_start_matches("File:")
                .trim_start_matches("FILE:")
                .trim()
                .trim_matches('`')
                .trim_matches('*')
                .trim();
            if is_valid_filename(clean) {
                pending_file_header = Some(clean.to_string());
            }
            i += 1;
            continue;
        }

        if (line.starts_with("**") || line.starts_with("__")) && (line.ends_with("**") || line.ends_with("__:") || line.ends_with("**:") || line.ends_with("__")) {
            let clean = line
                .trim_matches('*')
                .trim_matches('_')
                .trim_end_matches(':')
                .trim()
                .trim_matches('`');
            if is_valid_filename(clean) {
                pending_file_header = Some(clean.to_string());
            }
            i += 1;
            continue;
        }

        // 2. Detect code fence open
        if line.starts_with("```") {
            let fence_info = line.trim_start_matches('`').trim();
            let mut file_from_fence: Option<String> = None;

            if let Some(pos) = fence_info.find(':') {
                let candidate = fence_info[pos + 1..].trim().trim_matches('"').trim_matches('\'');
                if is_valid_filename(candidate) {
                    file_from_fence = Some(candidate.to_string());
                }
            } else if let Some(pos) = fence_info.find("file=") {
                let candidate = fence_info[pos + 5..].trim().trim_matches('"').trim_matches('\'');
                if is_valid_filename(candidate) {
                    file_from_fence = Some(candidate.to_string());
                }
            } else if fence_info.contains(' ') {
                let parts: Vec<&str> = fence_info.split_whitespace().collect();
                for part in parts {
                    let clean = part.trim().trim_matches('"').trim_matches('\'');
                    if is_valid_filename(clean) {
                        file_from_fence = Some(clean.to_string());
                        break;
                    }
                }
            } else if is_valid_filename(fence_info) {
                file_from_fence = Some(fence_info.to_string());
            }

            i += 1;
            let mut block_lines = Vec::new();
            while i < lines.len() && !lines[i].trim().starts_with("```") {
                block_lines.push(lines[i].to_string());
                i += 1;
            }
            if i < lines.len() && lines[i].trim().starts_with("```") {
                i += 1; // skip closing fence
            }

            // Determine target file
            let mut resolved_file = file_from_fence.or_else(|| pending_file_header.take());

            // Check if line 0 of block contains a file comment (e.g. "// index.html", "/* styles.css */", "<!-- index.html -->")
            if resolved_file.is_none() && !block_lines.is_empty() {
                let first_line = block_lines[0].trim();
                let comment_candidate = first_line
                    .trim_start_matches("//")
                    .trim_start_matches("/*")
                    .trim_start_matches("<!--")
                    .trim_start_matches('#')
                    .trim_end_matches("*/")
                    .trim_end_matches("-->")
                    .trim()
                    .trim_matches('`');
                if is_valid_filename(comment_candidate) {
                    resolved_file = Some(comment_candidate.to_string());
                    // Remove the comment header line so file is clean
                    block_lines.remove(0);
                }
            }

            collected_blocks.push((resolved_file, block_lines));
            continue;
        }

        i += 1;
    }

    // If there is only one block and no resolved file, use fallback_target_file
    if collected_blocks.len() == 1 && collected_blocks[0].0.is_none() {
        if let Some(fallback) = fallback_target_file {
            collected_blocks[0].0 = Some(fallback.to_string());
        }
    }

    // Write all resolved blocks
    for (file_opt, content_lines) in collected_blocks {
        if let Some(file_rel) = file_opt {
            let clean_path = file_rel.trim().trim_start_matches("./").replace('\\', "/");
            let full_target = worktree_dir.join(&clean_path);

            if let Some(parent) = full_target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let was_new = !full_target.exists();
            let old_line_count = if !was_new {
                std::fs::read_to_string(&full_target).map(|s| s.lines().count()).unwrap_or(0)
            } else {
                0
            };

            let content = content_lines.join("\n");
            let new_line_count = content_lines.len();

            std::fs::write(&full_target, &content)
                .map_err(|e| format!("Failed writing markdown file block {clean_path}: {e}"))?;

            results.push(AppliedDiffResult {
                file_path: clean_path,
                is_new: was_new,
                additions: new_line_count,
                deletions: old_line_count,
            });
        }
    }

    Ok(results)
}

fn is_valid_filename(s: &str) -> bool {
    let clean = s.trim().trim_matches('`').trim_matches('"').trim_matches('\'');
    if clean.is_empty() || clean.contains('\n') || clean.len() > 120 {
        return false;
    }
    // Must contain a period with an extension (e.g. .html, .js, .css, .py, .rs, .json, .toml)
    // and not be a prose sentence
    if let Some(dot_idx) = clean.rfind('.') {
        if dot_idx > 0 && dot_idx + 1 < clean.len() {
            let ext = &clean[dot_idx + 1..];
            if !ext.contains(' ') && ext.len() <= 10 && !clean.contains("..") {
                return true;
            }
        }
    }
    false
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

/// Normalizes a line for fuzzy comment/whitespace comparison:
/// Strips comment delimiters like `/*`, `*/`, `//`, `#`, and trailing/leading slashes and spaces.
fn normalize_fuzzy_line(line: &str) -> String {
    let trimmed = line.trim();
    let without_comments = trimmed
        .trim_start_matches("/*")
        .trim_end_matches("*/")
        .trim_start_matches("//")
        .trim_start_matches('#')
        .trim_matches('/')
        .trim();
    // Collapse internal whitespace
    without_comments.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Fallback: checks if lines match when punctuation/comment markers are normalized
fn find_fuzzy_comment_subslice(haystack: &[&str], needle: &[&str]) -> Option<usize> {
    let norm_needle: Vec<String> = needle.iter().map(|l| normalize_fuzzy_line(l)).collect();
    // If needle is only empty lines after normalization, don't false match
    if norm_needle.iter().all(|s| s.is_empty()) {
        return None;
    }
    if norm_needle.len() > haystack.len() {
        return None;
    }

    let norm_haystack: Vec<String> = haystack.iter().map(|l| normalize_fuzzy_line(l)).collect();

    'outer: for i in 0..=(norm_haystack.len() - norm_needle.len()) {
        for j in 0..norm_needle.len() {
            if !norm_needle[j].is_empty() && norm_haystack[i + j] != norm_needle[j] {
                continue 'outer;
            }
        }
        return Some(i);
    }
    None
}

/// Fallback: matches using unique non-empty first and last lines as anchors
fn find_anchor_subslice(haystack: &[&str], needle: &[&str]) -> Option<(usize, usize)> {
    let non_empty_needle: Vec<(usize, &str)> = needle
        .iter()
        .enumerate()
        .map(|(idx, l)| (idx, l.trim()))
        .filter(|(_, l)| !l.is_empty())
        .collect();

    if non_empty_needle.len() < 2 {
        return None;
    }

    let first_needle = non_empty_needle.first().unwrap().1;
    let last_needle = non_empty_needle.last().unwrap().1;

    let mut candidate: Option<(usize, usize)> = None;

    for i in 0..haystack.len() {
        if haystack[i].trim() == first_needle || normalize_fuzzy_line(haystack[i]) == normalize_fuzzy_line(first_needle) {
            // Look forward for matching last anchor within reasonable window
            let max_lookahead = (i + needle.len() + 10).min(haystack.len());
            for j in (i + 1)..max_lookahead {
                if haystack[j].trim() == last_needle || normalize_fuzzy_line(haystack[j]) == normalize_fuzzy_line(last_needle) {
                    if candidate.is_some() {
                        // Ambiguous multiple matches in file, reject anchor match to be safe
                        return None;
                    }
                    candidate = Some((i, j + 1));
                }
            }
        }
    }

    candidate
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

    #[test]
    fn test_file_folder_operations_delete_mkdir_move_rmdir() {
        let temp = std::env::temp_dir().join(format!("test_ops_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
        std::fs::create_dir_all(&temp).unwrap();

        // 1. Create a dummy file to delete
        std::fs::write(temp.join("obsolete.txt"), "delete me").unwrap();
        assert!(temp.join("obsolete.txt").exists());

        // 2. Create a file to move
        std::fs::write(temp.join("move_me.txt"), "moving this").unwrap();

        let response = r#"
<<<<<<< MKDIR: new_folder/sub_folder >>>>>>>
<<<<<<< MOVE: move_me.txt -> new_folder/sub_folder/moved.txt >>>>>>>
<<<<<<< DELETE: obsolete.txt >>>>>>>
<<<<<<< MKDIR: temp_to_delete >>>>>>>
<<<<<<< RMDIR: temp_to_delete >>>>>>>
"#;

        let results = apply_diff_blocks(&temp, response, None).unwrap();
        assert_eq!(results.len(), 5);

        // Verify obsolete.txt is deleted
        assert!(!temp.join("obsolete.txt").exists());

        // Verify folder created and file moved
        assert!(temp.join("new_folder/sub_folder/moved.txt").exists());
        assert_eq!(std::fs::read_to_string(temp.join("new_folder/sub_folder/moved.txt")).unwrap(), "moving this");
        assert!(!temp.join("move_me.txt").exists());

        // Verify temp_to_delete was removed
        assert!(!temp.join("temp_to_delete").exists());

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_markdown_file_blocks_parsing() {
        let temp = std::env::temp_dir().join(format!("test_md_diff_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
        std::fs::create_dir_all(&temp).unwrap();

        let md_response = r#"
Here is the code to build the website based on the design specification:

### `index.html`
```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Website</title>
    <link rel="stylesheet" href="styles.css">
</head>
<body>
    <h1>Welcome</h1>
    <script src="app.js"></script>
</body>
</html>
```

### File: styles.css
```css
body {
    font-family: sans-serif;
    background-color: #f0f0f0;
}
```

**app.js**:
```javascript
console.log("App loaded successfully");
```
"#;

        let results = apply_diff_blocks(&temp, md_response, None).unwrap();
        assert_eq!(results.len(), 3, "Must parse all 3 files from markdown code blocks");

        let files: Vec<String> = results.iter().map(|r| r.file_path.clone()).collect();
        assert!(files.contains(&"index.html".to_string()));
        assert!(files.contains(&"styles.css".to_string()));
        assert!(files.contains(&"app.js".to_string()));

        assert!(temp.join("index.html").exists());
        assert!(temp.join("styles.css").exists());
        assert!(temp.join("app.js").exists());

        let index_content = std::fs::read_to_string(temp.join("index.html")).unwrap();
        assert!(index_content.contains("<!DOCTYPE html>"));

        let js_content = std::fs::read_to_string(temp.join("app.js")).unwrap();
        assert!(js_content.contains("App loaded successfully"));

        let _ = std::fs::remove_dir_all(&temp);
    }
}
