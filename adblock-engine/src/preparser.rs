//! uBlock Origin's `!#if` / `!#else` / `!#endif` list pre-parsing (B5-T0-P6).
//!
//! adblock-rust (0.10.3, still 0.13.3) treats every `!` line as a comment, so all branches of a
//! conditional block were loaded — Firefox-only rules, uBO Lite (`ext_ubol`) exceptions and their
//! `!#else` counterparts together. This drops the branches that do not apply to Hodos.
//!
//! Ported (semantics, not code) from uBO 1.75.0 `static-filtering-parser.js :: preparser`:
//! - an expression is `token (( && | || ) token)*`, evaluated left to right, optionally wrapped in
//!   one pair of parentheses; a token may be prefixed with `!`;
//! - tokens map to an environment state; an unknown `cap_*` token is false, any other unknown
//!   token makes the whole expression unknown — and an unknown block is KEPT, as in uBO;
//! - `!#else` flips the innermost block; a line is discarded while any enclosing known block is.

/// The environment uBO reports on Chromium (`vAPI.webextFlavor.env`): Hodos is a Chromium
/// engine running full (MV2-style) uBO filtering, without HTML filtering or IP-address caps.
pub const HODOS_ENV: &[&str] = &["chromium", "ublock"];

/// uBO's `preparserTokens`: directive token -> environment state.
fn token_state(token: &str) -> Option<&'static str> {
    Some(match token {
        "ext_ublock" => "ublock",
        "ext_ubol" => "ubol",
        "ext_devbuild" => "devbuild",
        "env_brave" => "brave",
        "env_chromium" => "chromium",
        "env_edge" => "edge",
        "env_firefox" => "firefox",
        "env_legacy" => "legacy",
        "env_mobile" => "mobile",
        "env_mv3" => "mv3",
        "env_safari" => "safari",
        "cap_html_filtering" => "html_filtering",
        "cap_ipaddress" => "ipaddress",
        "false" => "false",
        "ext_abp" => "false",
        "adguard" => "adguard",
        "adguard_app_android" | "adguard_app_cli" | "adguard_app_ios" | "adguard_app_mac"
        | "adguard_app_windows" | "adguard_ext_android_cb" | "adguard_ext_safari" => "false",
        "adguard_ext_chromium" | "adguard_ext_opera" => "chromium",
        "adguard_ext_chromium_mv3" => "mv3",
        "adguard_ext_edge" => "edge",
        "adguard_ext_firefox" => "firefox",
        _ => return None,
    })
}

fn eval_token(token: &str, env: &[&str]) -> Option<bool> {
    let (not, name) = match token.strip_prefix('!') {
        Some(rest) => (true, rest),
        None => (false, token),
    };
    let state = match token_state(name) {
        Some(s) => s,
        None if name.starts_with("cap_") => "false",
        None => return None,
    };
    Some((state == "false" && not) || (env.contains(&state) != not))
}

/// Evaluate an `!#if` expression; `None` = unknown (the block is kept).
pub fn eval_expr(expr: &str, env: &[&str]) -> Option<bool> {
    let mut expr = expr.trim();
    if expr.starts_with('(') && expr.ends_with(')') {
        expr = &expr[1..expr.len() - 1];
    }
    let mut words = expr.split_whitespace();
    let first = words.next()?;
    if first.starts_with('&') || first.starts_with('|') {
        return None;
    }
    let mut result = eval_token(first, env)?;
    loop {
        let op = match words.next() {
            None => return Some(result),
            Some(op) => op,
        };
        let state = eval_token(words.next()?, env)?;
        result = match op {
            "||" => result || state,
            "&&" => result && state,
            _ => return None,
        };
    }
}

struct Block {
    known: bool,
    discard: bool,
}

/// Remove the lines that `!#if` blocks exclude for `env`. Directive lines are dropped too (they
/// are comments to the engine). Unbalanced directives behave as in uBO: a stray `!#else` or
/// `!#endif` is ignored, an unclosed `!#if` runs to the end.
pub fn prune(text: &str, env: &[&str]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut stack: Vec<Block> = Vec::new();
    let discarding = |stack: &Vec<Block>| stack.iter().any(|b| b.known && b.discard);
    for line in text.lines() {
        if let Some(rest) = directive(line, "if") {
            let r = eval_expr(rest, env);
            stack.push(Block { known: r.is_some(), discard: r == Some(false) });
            continue;
        }
        if directive(line, "else").is_some() {
            if let Some(top) = stack.last_mut() {
                top.discard = !top.discard;
            }
            continue;
        }
        if directive(line, "endif").is_some() {
            stack.pop();
            continue;
        }
        if !discarding(&stack) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// `!#<name>` followed by whitespace or end of line (uBO's `\b`); returns the rest.
fn directive<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let rest = line.strip_prefix("!#")?.strip_prefix(name)?;
    match rest.chars().next() {
        None => Some(rest),
        Some(c) if !(c.is_alphanumeric() || c == '_') => Some(rest),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions_follow_ubo() {
        let env = HODOS_ENV;
        assert_eq!(eval_expr("env_chromium", env), Some(true));
        assert_eq!(eval_expr("env_firefox", env), Some(false));
        assert_eq!(eval_expr("!env_mv3", env), Some(true));
        assert_eq!(eval_expr("ext_ubol", env), Some(false));
        assert_eq!(eval_expr("!cap_html_filtering", env), Some(true));
        assert_eq!(eval_expr("cap_user_stylesheet", env), Some(false), "unknown cap_ is false");
        assert_eq!(eval_expr("!false", env), Some(true));
        assert_eq!(eval_expr("env_firefox || env_chromium", env), Some(true));
        assert_eq!(eval_expr("env_chromium && !env_mobile", env), Some(true));
        assert_eq!(eval_expr("(env_chromium && env_mv3)", env), Some(false));
        assert_eq!(eval_expr("some_new_token", env), None, "unknown token -> unknown");
        assert_eq!(eval_expr("env_chromium &&", env), None);
        assert_eq!(eval_expr("&& env_chromium", env), None);
    }

    #[test]
    fn prune_keeps_only_applicable_branches() {
        let list = "a\n!#if env_firefox\nfirefox-only\n!#else\nnot-firefox\n!#endif\nb\n\
                    !#if !env_mv3\n!#if ext_ubol\nubol\n!#else\nfull-ubo\n!#endif\n!#endif\n\
                    !#if unknown_thing\nkept-unknown\n!#endif\nc\n";
        assert_eq!(prune(list, HODOS_ENV), "a\nnot-firefox\nb\nfull-ubo\nkept-unknown\nc\n");
    }

    #[test]
    fn a_discarded_outer_block_hides_its_inner_else() {
        let list = "!#if env_firefox\n!#if ext_ubol\nx\n!#else\ny\n!#endif\n!#endif\nz\n";
        assert_eq!(prune(list, HODOS_ENV), "z\n");
    }

    #[test]
    fn directive_needs_a_word_boundary() {
        assert!(directive("!#iffy", "if").is_none());
        assert_eq!(directive("!#if env_chromium", "if"), Some(" env_chromium"));
        assert_eq!(directive("!#endif", "endif"), Some(""));
        // Ordinary comments pass through.
        assert_eq!(prune("! comment\n||ads.example^\n", HODOS_ENV), "! comment\n||ads.example^\n");
    }
}
