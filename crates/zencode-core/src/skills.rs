use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectStack {
    Android,
    Flutter,
    IosSwift,
    Rust,
    NodeWeb,
    Python,
    Go,
    JavaMaven,
    Cplusplus,
    Custom(String),
    Generic,
}

impl ProjectStack {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Android => "Android (Kotlin / Jetpack Compose / Gradle)",
            Self::Flutter => "Flutter (Dart)",
            Self::IosSwift => "iOS / macOS (Swift / SwiftUI)",
            Self::Rust => "Rust (Cargo)",
            Self::NodeWeb => "Web / Node.js (TypeScript / JavaScript)",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::JavaMaven => "Java / Kotlin (Maven)",
            Self::Cplusplus => "C / C++ (CMake / Make)",
            Self::Custom(_) => "Custom Stack",
            Self::Generic => "Generic Project",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSkill {
    pub source_file: String,
    pub title: String,
    pub content: String,
}

/// Detect project stack from relative file paths present in the shallow tree or repository
pub fn detect_stack_from_files(files: &[String], prompt: &str) -> ProjectStack {
    let lower_prompt = prompt.to_lowercase();

    // Check prompt hints first if the user explicitly specifies the target platform
    if lower_prompt.contains("android") || lower_prompt.contains("jetpack compose") || lower_prompt.contains("apk") {
        return ProjectStack::Android;
    }
    if lower_prompt.contains("flutter") || lower_prompt.contains("dart") {
        return ProjectStack::Flutter;
    }
    if lower_prompt.contains("swiftui") || lower_prompt.contains("ios app") || lower_prompt.contains("xcode") {
        return ProjectStack::IosSwift;
    }

    // Inspect files in repository tree
    let mut has_android = false;
    let mut has_flutter = false;
    let mut has_ios = false;
    let mut has_rust = false;
    let mut has_node = false;
    let mut has_python = false;
    let mut has_go = false;
    let mut has_maven = false;
    let mut has_cmake = false;

    for f in files {
        let p = f.replace('\\', "/").to_lowercase();
        let filename = p.rsplit('/').next().unwrap_or(&p);

        if filename == "androidmanifest.xml" 
            || filename == "build.gradle" 
            || filename == "build.gradle.kts" 
            || filename == "settings.gradle" 
            || filename == "settings.gradle.kts"
            || p.contains("app/src/main/java")
            || p.contains("app/src/main/kotlin") {
            has_android = true;
        }
        if filename == "pubspec.yaml" || filename == "pubspec.lock" {
            has_flutter = true;
        }
        if filename == "package.swift" || filename == "podfile" || filename.ends_with(".xcodeproj") || filename.ends_with(".xcworkspace") {
            has_ios = true;
        }
        if filename == "cargo.toml" {
            has_rust = true;
        }
        if filename == "package.json" || filename == "pnpm-lock.yaml" || filename == "yarn.lock" || filename == "bun.lockb" {
            has_node = true;
        }
        if filename == "pyproject.toml" || filename == "requirements.txt" || filename == "pipfile" || filename == "setup.py" {
            has_python = true;
        }
        if filename == "go.mod" || filename == "go.sum" {
            has_go = true;
        }
        if filename == "pom.xml" {
            has_maven = true;
        }
        if filename == "cmakelists.txt" || filename == "makefile" {
            has_cmake = true;
        }
    }

    // Flutter projects contain android/ and ios/ subdirectories, prioritize Flutter
    if has_flutter {
        return ProjectStack::Flutter;
    }
    if has_android {
        return ProjectStack::Android;
    }
    if has_ios {
        return ProjectStack::IosSwift;
    }
    if has_rust {
        return ProjectStack::Rust;
    }
    if has_node {
        return ProjectStack::NodeWeb;
    }
    if has_python {
        return ProjectStack::Python;
    }
    if has_go {
        return ProjectStack::Go;
    }
    if has_maven {
        return ProjectStack::JavaMaven;
    }
    if has_cmake {
        return ProjectStack::Cplusplus;
    }

    ProjectStack::Generic
}

/// Detect project stack from filesystem path
pub fn detect_stack_from_dir(dir: &Path) -> ProjectStack {
    if !dir.exists() {
        return ProjectStack::Generic;
    }

    if dir.join("pubspec.yaml").exists() {
        return ProjectStack::Flutter;
    }
    if dir.join("build.gradle.kts").exists()
        || dir.join("build.gradle").exists()
        || dir.join("settings.gradle.kts").exists()
        || dir.join("settings.gradle").exists()
        || dir.join("app/src/main/AndroidManifest.xml").exists()
    {
        return ProjectStack::Android;
    }
    if dir.join("Package.swift").exists() || dir.join("Podfile").exists() {
        return ProjectStack::IosSwift;
    }
    if dir.join("Cargo.toml").exists() {
        return ProjectStack::Rust;
    }
    if dir.join("package.json").exists() {
        return ProjectStack::NodeWeb;
    }
    if dir.join("pyproject.toml").exists() || dir.join("requirements.txt").exists() || dir.join("Pipfile").exists() {
        return ProjectStack::Python;
    }
    if dir.join("go.mod").exists() {
        return ProjectStack::Go;
    }
    if dir.join("pom.xml").exists() {
        return ProjectStack::JavaMaven;
    }
    if dir.join("CMakeLists.txt").exists() || dir.join("Makefile").exists() {
        return ProjectStack::Cplusplus;
    }

    ProjectStack::Generic
}

/// Scans the workspace directory for custom skill/rule markdown documents:
/// - .zencode/skills/*.md
/// - .zencode/rules/*.md
/// - .zencode/guidelines.md
/// - .cursorrules
/// - AGENTS.md
/// - CLAUDE.md
pub fn discover_workspace_skills(dir: &Path) -> Vec<WorkspaceSkill> {
    let mut skills = Vec::new();
    if !dir.exists() {
        return skills;
    }

    // 1. Check .zencode/skills/
    let zencode_skills = dir.join(".zencode").join("skills");
    if zencode_skills.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&zencode_skills) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("md") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("skill.md");
                        skills.push(WorkspaceSkill {
                            source_file: format!(".zencode/skills/{}", filename),
                            title: format!("Custom Skill: {}", filename.trim_end_matches(".md")),
                            content,
                        });
                    }
                }
            }
        }
    }

    // 2. Check .zencode/rules/
    let zencode_rules = dir.join(".zencode").join("rules");
    if zencode_rules.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&zencode_rules) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("md") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("rule.md");
                        skills.push(WorkspaceSkill {
                            source_file: format!(".zencode/rules/{}", filename),
                            title: format!("Workspace Rule: {}", filename.trim_end_matches(".md")),
                            content,
                        });
                    }
                }
            }
        }
    }

    // 3. Check individual standard rule files
    let standard_files = [
        (".zencode/guidelines.md", "Project Guidelines"),
        (".cursorrules", "Cursor Rules Specification"),
        ("AGENTS.md", "Agent Directives"),
        ("CLAUDE.md", "Project Directives"),
    ];

    for (rel_path, title) in &standard_files {
        let p = dir.join(rel_path);
        if p.is_file() {
            if let Ok(content) = std::fs::read_to_string(&p) {
                skills.push(WorkspaceSkill {
                    source_file: rel_path.to_string(),
                    title: title.to_string(),
                    content,
                });
            }
        }
    }

    skills
}

/// Returns dynamic built-in stack guidelines and anti-pattern shields when no custom overrides exist
pub fn get_stack_guideline(stack: &ProjectStack) -> Option<String> {
    match stack {
        ProjectStack::Android => Some(
            r#"### ANDROID (KOTLIN / JETPACK COMPOSE / GRADLE) STACK SPECIFICATION
1. ARCHITECTURAL PATTERNS:
   - Modern UI: Jetpack Compose with Material 3 (androidx.compose.material3).
   - Architecture: Clean Architecture + MVI/MVVM with StateFlow & ViewModel.
   - Concurrency: Kotlin Coroutines with viewModelScope and Flow operators.
   - Lifecycle: androidx.lifecycle.compose.collectAsStateWithLifecycle() for UI observation.
   - Build Tooling: Gradle Wrapper (./gradlew or gradlew.bat). Key tasks:
     * Compile/Check: `./gradlew assembleDebug`
     * Unit Tests: `./gradlew testDebugUnitTest`
     * Lint: `./gradlew lintDebug`

2. ANDROID ANTI-PATTERN SHIELD:
   - NEVER execute disk I/O, database access (Room), or network calls on Dispatchers.Main. Always use withContext(Dispatchers.IO).
   - NEVER pass or hold an Activity Context inside static references, singletons, or ViewModels. Memory leak risk.
   - IN JETPACK COMPOSE:
     * Never perform side-effects directly inside the body of a Composable function. Always wrap them in LaunchedEffect, DisposableEffect, or side-effect handlers.
     * Always remember UI state across configuration changes using `rememberSaveable { mutableStateOf(...) }` or hoist state into a ViewModel.
     * Use Modifier properly: Modifier order matters in Compose (e.g. padding before background vs after).
     * Avoid unnecessary recompositions: Use derivedStateOf for computations derived from other state.
   - Always verify XML manifest permissions and exported flags (android:exported="true|false" required for Android 12+ components).
"#.to_string()
        ),
        ProjectStack::Flutter => Some(
            r#"### FLUTTER (DART) STACK SPECIFICATION
1. ARCHITECTURAL PATTERNS:
   - UI: Declarative Flutter Widget Tree with Material 3.
   - State Management: Riverpod, Bloc, or Provider with unidirectional data flow.
   - Tooling:
     * Code Analysis: `flutter analyze`
     * Test Suite: `flutter test`
     * Build: `flutter build apk` or `flutter build web`

2. FLUTTER ANTI-PATTERN SHIELD:
   - Never call `setState()` after `dispose()` has been invoked; always check `if (!mounted) return;` before calling `setState()` across async gaps.
   - Avoid monolithic build() methods; decompose large screens into small `StatelessWidget` sub-components to limit rebuild scopes.
   - Do not perform expensive computations or I/O directly in build(); use compute() isolates or async repositories.
   - Always specify const constructors where possible (`const SizedBox(...)`) to enable widget subtree memoization.
"#.to_string()
        ),
        ProjectStack::IosSwift => Some(
            r#"### IOS / MACOS (SWIFT / SWIFTUI) STACK SPECIFICATION
1. ARCHITECTURAL PATTERNS:
   - Modern UI: Declarative SwiftUI with NavigationStack.
   - Concurrency: Swift Concurrency (`async/await`, `Task`, `actors`, `@MainActor`).
   - Tooling:
     * Build / Test: `swift test` or `xcodebuild -scheme <App> test`

2. SWIFT ANTI-PATTERN SHIELD:
   - Never capture `self` strongly in long-lived escaping closures or Combine pipelines. Use `[weak self]` to avoid retain cycles.
   - Always ensure UI state mutations are isolated to `@MainActor`.
   - Never block the cooperative thread pool with synchronous I/O or sleep calls.
"#.to_string()
        ),
        ProjectStack::Python => Some(
            r#"### PYTHON STACK SPECIFICATION
1. ARCHITECTURAL PATTERNS:
   - Tooling: Python 3.10+, pytest, ruff / flake8 / black.
   - Verification: `pytest` or `python -m unittest`
2. PYTHON ANTI-PATTERN SHIELD:
   - Never use mutable default arguments (e.g. `def foo(items=[])`). Use `items=None` and initialize inside.
   - Always manage I/O with context managers (`with open(...) as f:`).
   - Use strict type annotations from `typing` for function signatures and public APIs.
"#.to_string()
        ),
        ProjectStack::Rust => Some(
            r#"### RUST (CARGO) STACK SPECIFICATION
1. ARCHITECTURAL PATTERNS:
   - Error handling: Use Result<T, E> with custom Error enums (thiserror) or anyhow for applications.
   - Tooling: `cargo check`, `cargo test --workspace`, `cargo clippy`.
2. RUST ANTI-PATTERN SHIELD:
   - Zero `.unwrap()` or `.expect()` in production error paths; propagate with `?`.
   - Keep lifetimes simple and prefer owned types or Arc/Rc over complex nested references unless hot loops demand it.
"#.to_string()
        ),
        _ => None,
    }
}

/// Formats the complete dynamic stack context (detected guidelines + custom workspace skills)
/// to inject into Gemini Director and Groq Worker prompts.
pub fn format_dynamic_stack_context(
    stack: &ProjectStack,
    workspace_skills: &[WorkspaceSkill],
) -> String {
    let mut out = String::new();

    out.push_str(&format!("DETECTED PROJECT ECOSYSTEM: {}\n", stack.display_name()));

    if let Some(guideline) = get_stack_guideline(stack) {
        out.push_str(&guideline);
        out.push('\n');
    }

    if !workspace_skills.is_empty() {
        out.push_str("WORKSPACE SKILLS & PROJECT DIRECTIVES:\n");
        for skill in workspace_skills {
            out.push_str(&format!("--- [{}] ({}) ---\n{}\n\n", skill.title, skill.source_file, skill.content.trim()));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_android_from_files() {
        let files = vec![
            "app/build.gradle.kts".to_string(),
            "app/src/main/AndroidManifest.xml".to_string(),
            "settings.gradle.kts".to_string(),
        ];
        let stack = detect_stack_from_files(&files, "add a login screen");
        assert_eq!(stack, ProjectStack::Android);
    }

    #[test]
    fn test_detect_flutter_from_files() {
        let files = vec![
            "pubspec.yaml".to_string(),
            "lib/main.dart".to_string(),
        ];
        let stack = detect_stack_from_files(&files, "build a counter");
        assert_eq!(stack, ProjectStack::Flutter);
    }

    #[test]
    fn test_detect_from_prompt_hint() {
        let files: Vec<String> = vec![];
        let stack = detect_stack_from_files(&files, "Create an Android Jetpack Compose app");
        assert_eq!(stack, ProjectStack::Android);
    }

    #[test]
    fn test_guideline_generation() {
        let guideline = get_stack_guideline(&ProjectStack::Android);
        assert!(guideline.is_some());
        let text = guideline.unwrap();
        assert!(text.contains("JETPACK COMPOSE"));
        assert!(text.contains("ANTI-PATTERN SHIELD"));
    }
}
