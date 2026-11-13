/// ZenCode Design Document & UI Design System Engine.
/// Provides curated, battle-tested design tokens, elevation curves, typography scales,
/// component blueprints, and anti-pattern shields to elevate generated UI to frontier standards.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesignSystemArchetype {
    DeveloperTool,
    ModernSaaS,
    DarkCyber,
    MinimalistEditorial,
}

#[derive(Debug, Clone)]
pub struct DesignSystemPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub font_headings: &'static str,
    pub font_body: &'static str,
    pub font_code: &'static str,
    pub bg_color: &'static str,
    pub surface_color: &'static str,
    pub text_primary: &'static str,
    pub text_secondary: &'static str,
    pub accent_color: &'static str,
    pub border_subtle: &'static str,
    pub css_variables_template: &'static str,
    pub component_blueprints: &'static str,
}

pub static DEVELOPER_TOOL_PRESET: DesignSystemPreset = DesignSystemPreset {
    id: "developer-tool",
    name: "Modern Developer Tool (ZenCode Standard)",
    description: "Refined aesthetic for technical tools: warm cream background, crisp charcoal typography, burnt terracotta accent, dark CLI terminal blocks, and subtle 1px border containers.",
    font_headings: "'Plus Jakarta Sans', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
    font_body: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
    font_code: "'JetBrains Mono', 'Fira Code', monospace",
    bg_color: "#faf9f5",
    surface_color: "#ffffff",
    text_primary: "#141413",
    text_secondary: "#605f5b",
    accent_color: "#c96442",
    border_subtle: "rgba(20, 20, 19, 0.12)",
    css_variables_template: r#":root {
  --bg-primary: #faf9f5;
  --bg-surface: #ffffff;
  --bg-surface-elevated: #f4f3ef;
  --bg-terminal: #0f0f10;
  --text-primary: #141413;
  --text-secondary: #605f5b;
  --text-muted: #878682;
  --accent: #c96442;
  --accent-hover: #b35535;
  --accent-light: rgba(201, 100, 66, 0.08);
  --border-subtle: rgba(20, 20, 19, 0.1);
  --border-focus: rgba(201, 100, 66, 0.4);
  --shadow-sm: 0 1px 2px rgba(20, 20, 19, 0.04);
  --shadow-md: 0 4px 12px -2px rgba(20, 20, 19, 0.06), 0 2px 4px -1px rgba(20, 20, 19, 0.03);
  --shadow-lg: 0 12px 24px -4px rgba(20, 20, 19, 0.08), 0 4px 8px -2px rgba(20, 20, 19, 0.04);
  --radius-sm: 6px;
  --radius-md: 10px;
  --radius-lg: 16px;
  --transition-smooth: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  --font-sans: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  --font-mono: 'JetBrains Mono', monospace;
}"#,
    component_blueprints: r#"### Curated Component Blueprints:
1. Navigation Bar:
   - Sticky top bar with backdrop-filter: blur(12px) and border-bottom: 1px solid var(--border-subtle).
   - Brand split wordmark (e.g. Zen in black, Code in terracotta accent).
   - Navigation links with hover transition: color var(--transition-smooth).
   - Pill-shaped Primary CTA button with box-shadow: var(--shadow-sm).

2. Hero Section:
   - Centered or asymmetric split layout with bold display heading (font-weight: 700; letter-spacing: -0.03em).
   - Version/Status pill badge (e.g. 'v0.1.0 Public Beta') with accent background and border.
   - Dual Call-to-Action buttons (Primary solid accent + Secondary outline).
   - Interactive IDE/Terminal Preview container with window control dots (red, yellow, green) and syntax-highlighted code.

3. Step-by-Step / Feature Cards:
   - Clean white rectangular surface (background: var(--bg-surface)) with thin border (1px solid var(--border-subtle)).
   - Hover lift: transform: translateY(-3px) and box-shadow: var(--shadow-md).
   - Active step card highlighting with border-color: var(--accent) and soft background tint.

4. Dark Terminal Block:
   - Deep black container (background: var(--bg-terminal)) with monospace green/white execution text.
   - Live interactive state: updates output dynamically when step cards are clicked."#,
};

pub static MODERN_SAAS_PRESET: DesignSystemPreset = DesignSystemPreset {
    id: "modern-saas",
    name: "Modern Clean SaaS",
    description: "Vercel & Linear-inspired clean SaaS interface: crisp neutral background, indigo/electric blue accent, subtle bento grids, and soft multi-layered elevation.",
    font_headings: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
    font_body: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
    font_code: "'JetBrains Mono', monospace",
    bg_color: "#ffffff",
    surface_color: "#f8fafc",
    text_primary: "#0f172a",
    text_secondary: "#475569",
    accent_color: "#4f46e5",
    border_subtle: "#e2e8f0",
    css_variables_template: r#":root {
  --bg-primary: #ffffff;
  --bg-surface: #f8fafc;
  --bg-surface-elevated: #ffffff;
  --bg-terminal: #0f172a;
  --text-primary: #0f172a;
  --text-secondary: #475569;
  --text-muted: #94a3b8;
  --accent: #4f46e5;
  --accent-hover: #4338ca;
  --accent-light: rgba(79, 70, 229, 0.08);
  --border-subtle: #e2e8f0;
  --border-focus: rgba(79, 70, 229, 0.4);
  --shadow-sm: 0 1px 3px rgba(15, 23, 42, 0.05);
  --shadow-md: 0 4px 6px -1px rgba(15, 23, 42, 0.08), 0 2px 4px -2px rgba(15, 23, 42, 0.04);
  --shadow-lg: 0 20px 25px -5px rgba(15, 23, 42, 0.1), 0 8px 10px -6px rgba(15, 23, 42, 0.05);
  --radius-sm: 6px;
  --radius-md: 12px;
  --radius-lg: 20px;
  --transition-smooth: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  --font-sans: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
  --font-mono: 'JetBrains Mono', monospace;
}"#,
    component_blueprints: r#"### Curated Component Blueprints:
1. Bento-Grid Layout:
   - Grid layout with gap: 24px and varied card spans (e.g. col-span-2 for hero card).
   - Glassmorphism surface: background: rgba(255, 255, 255, 0.85); backdrop-filter: blur(16px).
   - Subtle top-border highlight (border-top: 1px solid rgba(255, 255, 255, 0.6)).

2. Hero with Gradient Radial Glow:
   - Radial background gradient: radial-gradient(circle at 50% 0%, rgba(79, 70, 229, 0.12), transparent 70%).
   - Sleek pill badge with animated border shine.
   - High-contrast primary CTA button with inset top highlight shadow."#,
};

pub static DARK_CYBER_PRESET: DesignSystemPreset = DesignSystemPreset {
    id: "dark-cyber",
    name: "Dark Modern Obsidian / Cyber",
    description: "Deep obsidian dark mode interface with neon cyan/emerald accents, glowing subtle borders, and developer-grade dark containers.",
    font_headings: "'Space Grotesk', 'Inter', sans-serif",
    font_body: "'Inter', sans-serif",
    font_code: "'JetBrains Mono', monospace",
    bg_color: "#0a0a0c",
    surface_color: "#121216",
    text_primary: "#f4f4f6",
    text_secondary: "#9494a0",
    accent_color: "#10b981",
    border_subtle: "rgba(255, 255, 255, 0.08)",
    css_variables_template: r#":root {
  --bg-primary: #0a0a0c;
  --bg-surface: #121216;
  --bg-surface-elevated: #1a1a22;
  --bg-terminal: #050507;
  --text-primary: #f4f4f6;
  --text-secondary: #9494a0;
  --text-muted: #5e5e6e;
  --accent: #10b981;
  --accent-hover: #059669;
  --accent-light: rgba(16, 185, 129, 0.12);
  --border-subtle: rgba(255, 255, 255, 0.08);
  --border-focus: rgba(16, 185, 129, 0.5);
  --shadow-sm: 0 1px 3px rgba(0, 0, 0, 0.5);
  --shadow-md: 0 4px 16px rgba(0, 0, 0, 0.6);
  --shadow-lg: 0 12px 32px rgba(0, 0, 0, 0.8), 0 0 24px rgba(16, 185, 129, 0.15);
  --radius-sm: 6px;
  --radius-md: 10px;
  --radius-lg: 16px;
  --transition-smooth: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  --font-sans: 'Inter', sans-serif;
  --font-mono: 'JetBrains Mono', monospace;
}"#,
    component_blueprints: r#"### Curated Component Blueprints:
1. Glowing Borders & Dark Glass:
   - Subtle inner box-shadow with accent colored glow on hover.
   - Glassmorphic translucent surface cards with border: 1px solid var(--border-subtle).
   - High-contrast neon badges with pulse indicator dot."#,
};

pub static MINIMALIST_EDITORIAL_PRESET: DesignSystemPreset = DesignSystemPreset {
    id: "minimalist-editorial",
    name: "Minimalist Editorial / Studio",
    description: "Classic luxury and editorial clarity: warm off-white, deep charcoal typography, refined serif display headings, high whitespace, and bespoke simplicity.",
    font_headings: "'Playfair Display', 'Newsreader', Georgia, serif",
    font_body: "'Inter', system-ui, sans-serif",
    font_code: "'JetBrains Mono', monospace",
    bg_color: "#faf8f5",
    surface_color: "#ffffff",
    text_primary: "#181817",
    text_secondary: "#5c5b57",
    accent_color: "#2c2a26",
    border_subtle: "#e7e5e0",
    css_variables_template: r#":root {
  --bg-primary: #faf8f5;
  --bg-surface: #ffffff;
  --bg-surface-elevated: #f3f1ec;
  --bg-terminal: #1c1b18;
  --text-primary: #181817;
  --text-secondary: #5c5b57;
  --text-muted: #8c8a83;
  --accent: #2c2a26;
  --accent-hover: #000000;
  --accent-light: rgba(44, 42, 38, 0.06);
  --border-subtle: #e7e5e0;
  --border-focus: #181817;
  --shadow-sm: 0 1px 2px rgba(24, 24, 23, 0.03);
  --shadow-md: 0 6px 16px rgba(24, 24, 23, 0.04);
  --shadow-lg: 0 16px 32px rgba(24, 24, 23, 0.06);
  --radius-sm: 4px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --transition-smooth: all 0.25s ease;
  --font-serif: 'Playfair Display', Georgia, serif;
  --font-sans: 'Inter', system-ui, sans-serif;
  --font-mono: 'JetBrains Mono', monospace;
}"#,
    component_blueprints: r#"### Curated Component Blueprints:
1. Editorial Typography & Layout:
   - Elegant serif display titles with letter-spacing: -0.02em.
   - Generous section padding (padding: 120px 0).
   - Minimalist divider lines (1px solid var(--border-subtle)) separating content blocks."#,
};

/// Detects the most appropriate design system preset based on prompt keywords.
pub fn detect_design_system_intent(prompt: &str) -> Option<&'static DesignSystemPreset> {
    let lower = prompt.to_lowercase();
    let is_ui_task = lower.contains("website")
        || lower.contains("landing page")
        || lower.contains("web app")
        || lower.contains("ui")
        || lower.contains("frontend")
        || lower.contains("dashboard")
        || lower.contains("html")
        || lower.contains("css")
        || lower.contains("design")
        || lower.contains("zencode");

    if !is_ui_task {
        return None;
    }

    if lower.contains("dark") || lower.contains("cyber") || lower.contains("neon") || lower.contains("matrix") {
        Some(&DARK_CYBER_PRESET)
    } else if lower.contains("editorial") || lower.contains("magazine") || lower.contains("serif") || lower.contains("minimal") && lower.contains("blog") {
        Some(&MINIMALIST_EDITORIAL_PRESET)
    } else if lower.contains("saas") || lower.contains("b2b") || lower.contains("cloud") || lower.contains("indigo") {
        Some(&MODERN_SAAS_PRESET)
    } else {
        // Default to the flagship Developer Tool (ZenCode standard)
        Some(&DEVELOPER_TOOL_PRESET)
    }
}

/// Injects the complete design system specification and component blueprints into prompts.
pub fn format_design_system_guideline(preset: &DesignSystemPreset) -> String {
    format!(
        "### CURATED DESIGN SYSTEM GUIDELINE: {}\n\
        Description: {}\n\n\
        CSS Custom Properties Baseline:\n```css\n{}\n```\n\n\
        {}\n\n\
        DESIGN IMPLEMENTATION RULES:\n\
        1. Always define the full CSS variables block in :root inside the stylesheet.\n\
        2. Reference variables throughout components (e.g. background: var(--bg-surface); color: var(--text-primary); border: 1px solid var(--border-subtle)).\n\
        3. Never use raw #000 or harsh pure colors without variable tokens.\n\
        4. Apply smooth transitions (var(--transition-smooth)) and hover lifts (transform: translateY(-2px)).\n",
        preset.name,
        preset.description,
        preset.css_variables_template,
        preset.component_blueprints
    )
}

/// The Anti-Pattern Shield: explicitly forbids amateur 2012 CSS mistakes.
pub fn anti_pattern_shield() -> &'static str {
    "ANTI-PATTERN SHIELD (STRICTLY FORBIDDEN CODING PRACTICES):
1. NO raw browser dialogs: NEVER use `alert()`, `prompt()`, or `confirm()`. Use modern inline toasts, badges, or modals.
2. NO missing DOM null-checks: In JavaScript, ALWAYS wrap querySelector / getElementById calls defensively (e.g. `const el = document.getElementById('btn'); if (el) { el.addEventListener(...); }`).
3. NO un-reset buttons or inputs: Buttons must have `cursor: pointer; font-family: inherit; border: none; outline: none;`.
4. NO rigid pixel viewports: Never write fixed widths like `width: 1200px` without `max-width: 100%`. Use responsive `clamp()` or percentages.
5. NO harsh pure black shadows: Never write `box-shadow: 0 5px 15px #000`. Use layered ambient shadows with rgba (e.g. `rgba(0, 0, 0, 0.05)`).
6. NO unclosed HTML tags or mismatched IDs between JavaScript and HTML."
}

/// Pre-generation Reflection Checklist: forces model to self-verify contracts before output.
pub fn reflection_checklist() -> &'static str {
    "PRE-GENERATION VERIFICATION CHECKLIST (MANDATORY):
Before outputting code, verify:
- [ ] Contract Alignment: Every element accessed in JavaScript via getElementById or querySelector exists with an IDENTICAL id or class in the HTML.
- [ ] Asset Paths: All images, stylesheets, and scripts referenced (e.g. styles.css, app.js, logo.png, favicon.png) use valid relative paths matching files in the worktree.
- [ ] Zero Stubs: The code is 100% complete, fully implemented, with all styling, sections, and events wired up."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_design_system_intent() {
        let p1 = "Build this website based on zencode marketing strategy";
        let d1 = detect_design_system_intent(p1).unwrap();
        assert_eq!(d1.id, "developer-tool");

        let p2 = "Create a dark cyber terminal dashboard";
        let d2 = detect_design_system_intent(p2).unwrap();
        assert_eq!(d2.id, "dark-cyber");

        let p3 = "Create a clean SaaS landing page for my product";
        let d3 = detect_design_system_intent(p3).unwrap();
        assert_eq!(d3.id, "modern-saas");

        let p4 = "Fix bug in calculate_sum function in math.rs";
        let d4 = detect_design_system_intent(p4);
        assert!(d4.is_none());
    }

    #[test]
    fn test_format_guideline_contains_vars() {
        let guide = format_design_system_guideline(&DEVELOPER_TOOL_PRESET);
        assert!(guide.contains("--bg-primary"));
        assert!(guide.contains("--accent"));
        assert!(guide.contains("Curated Component Blueprints"));
    }
}
