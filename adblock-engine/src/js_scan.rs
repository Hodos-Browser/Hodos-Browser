//! A JavaScript *top-level* scanner, just enough to read uBlock Origin's scriptlet modules
//! (B5-T0-P6).
//!
//! It does not parse JavaScript. It tokenizes well enough to know where strings, template
//! literals, regular expressions and comments begin and end, so that brace depth is right,
//! and then recognises the handful of top-level statement shapes uBO uses to declare and
//! register scriptlets. Anything else at the top level is skipped; any *registration* in a
//! shape it does not recognise is an error, so a layout change in uBO fails loudly instead
//! of yielding a silently smaller set.
//!
//! The extracted function/class text is the exact source slice from `function`/`class` to
//! the closing brace — which is what `Function.prototype.toString()` returns, and so what
//! uBO itself injects.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Ident,
    Punct,
    /// Decoded string value.
    Str(String),
    /// Template literal; `Some(text)` when it has no `${}` substitution.
    Template(Option<String>),
    Regex,
    Num,
}

#[derive(Debug, Clone)]
pub struct Token<'a> {
    pub kind: Kind,
    pub text: &'a str,
    pub start: usize,
    pub end: usize,
}

impl Token<'_> {
    fn is(&self, s: &str) -> bool {
        matches!(self.kind, Kind::Ident | Kind::Punct) && self.text == s
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScanError(pub String);

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn err<T>(pos: usize, msg: impl std::fmt::Display) -> Result<T, ScanError> {
    Err(ScanError(format!("at byte {}: {}", pos, msg)))
}

/// Punctuators, longest first so the greedy match is right.
const PUNCTS: &[&str] = &[
    ">>>=", "...", "===", "!==", "**=", "<<=", ">>=", ">>>", "&&=", "||=", "??=",
    "=>", "==", "!=", "<=", ">=", "&&", "||", "??", "?.", "++", "--", "+=", "-=", "*=", "/=",
    "%=", "&=", "|=", "^=", "<<", ">>", "**",
    "{", "}", "(", ")", "[", "]", ";", ",", "<", ">", "+", "-", "*", "/", "%", "&", "|", "^",
    "!", "~", "?", ":", "=", ".", "@",
];

/// After these keywords an expression starts, so `/` begins a regular expression.
const REGEX_AFTER_KEYWORDS: &[&str] = &[
    "return", "typeof", "instanceof", "in", "of", "new", "delete", "void", "throw", "case",
    "do", "else", "yield", "await",
];

/// Whether a `/` after `prev` starts a regex literal (true) or is division (false).
///
/// The classic ambiguity is resolved by the previous significant token: after a value
/// (identifier, number, string, `)`, `]`, postfix `++`/`--`) it is division; after an
/// operator, an opening bracket, a keyword that takes an expression, or `}` (in uBO's code a
/// `}` before `/` ends a block, not an object literal) it is a regex.
fn slash_starts_regex(prev: Option<&Token>) -> bool {
    match prev {
        None => true,
        Some(t) => match &t.kind {
            Kind::Punct => !matches!(t.text, ")" | "]" | "++" | "--"),
            Kind::Ident => REGEX_AFTER_KEYWORDS.contains(&t.text),
            _ => false,
        },
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$' || c == '#' || c as u32 > 0x7f
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$' || c as u32 > 0x7f
}

/// Deepest `${ … }` nesting / literal nesting we follow. uBO's code nests a few levels; the
/// cap turns hostile input (this runs before the hash check) into an error, not a stack
/// overflow.
const MAX_NESTING: usize = 64;

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    /// Current template-substitution nesting depth.
    depth: usize,
}

impl<'a> Lexer<'a> {
    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.src[self.pos..].chars().nth(n)
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    /// Skip whitespace and comments.
    fn skip_trivia(&mut self) -> Result<(), ScanError> {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() || c == '\u{feff}' => {
                    self.bump();
                }
                Some('/') if self.peek_at(1) == Some('/') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' || c == '\r' || c == '\u{2028}' || c == '\u{2029}' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some('/') if self.peek_at(1) == Some('*') => {
                    let start = self.pos;
                    match self.src[self.pos + 2..].find("*/") {
                        Some(i) => self.pos += 2 + i + 2,
                        None => return err(start, "unterminated block comment"),
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    /// Read a quoted string starting at the opening quote; returns the decoded value.
    fn string(&mut self, quote: char) -> Result<String, ScanError> {
        let start = self.pos;
        self.bump();
        let mut out = String::new();
        loop {
            match self.bump() {
                None | Some('\n') | Some('\r') => return err(start, "unterminated string"),
                Some(c) if c == quote => return Ok(out),
                Some('\\') => self.escape(start, &mut out)?,
                Some(c) => out.push(c),
            }
        }
    }

    /// Decode one escape sequence (the backslash is already consumed).
    fn escape(&mut self, start: usize, out: &mut String) -> Result<(), ScanError> {
        let c = match self.bump() {
            None => return err(start, "unterminated escape"),
            Some(c) => c,
        };
        match c {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'v' => out.push('\u{b}'),
            '0' if !self.peek().is_some_and(|d| d.is_ascii_digit()) => out.push('\0'),
            'x' => out.push(self.hex_char(start, 2)?),
            'u' => {
                if self.peek() == Some('{') {
                    self.bump();
                    let s = self.pos;
                    while self.peek().is_some_and(|d| d.is_ascii_hexdigit()) {
                        self.bump();
                    }
                    let v = u32::from_str_radix(&self.src[s..self.pos], 16)
                        .ok()
                        .and_then(char::from_u32);
                    if self.bump() != Some('}') || v.is_none() {
                        return err(start, "bad \\u{} escape");
                    }
                    out.push(v.unwrap());
                } else {
                    out.push(self.hex_char(start, 4)?);
                }
            }
            '\r' => {
                if self.peek() == Some('\n') {
                    self.bump();
                }
            }
            '\n' | '\u{2028}' | '\u{2029}' => {}
            c if c.is_ascii_digit() => return err(start, "legacy octal escape not supported"),
            c => out.push(c),
        }
        Ok(())
    }

    fn hex_char(&mut self, start: usize, n: usize) -> Result<char, ScanError> {
        let s = self.pos;
        for _ in 0..n {
            if !self.bump().is_some_and(|d| d.is_ascii_hexdigit()) {
                return err(start, "bad hex escape");
            }
        }
        u32::from_str_radix(&self.src[s..self.pos], 16)
            .ok()
            .and_then(char::from_u32)
            .map_or_else(|| err(start, "bad hex escape value"), Ok)
    }

    /// Read a template literal starting at the backtick. Substitutions are tokenized
    /// recursively so strings, regexes and braces inside them cannot end the template early.
    fn template(&mut self) -> Result<Option<String>, ScanError> {
        let start = self.pos;
        self.bump();
        let mut out = String::new();
        let mut plain = true;
        loop {
            match self.bump() {
                None => return err(start, "unterminated template literal"),
                Some('`') => return Ok(if plain { Some(out) } else { None }),
                Some('\\') => self.escape(start, &mut out)?,
                Some('$') if self.peek() == Some('{') => {
                    self.bump();
                    plain = false;
                    self.depth += 1;
                    if self.depth > MAX_NESTING {
                        return err(start, "template substitutions nested too deeply");
                    }
                    self.code_until_close_brace(start)?;
                    self.depth -= 1;
                }
                Some(c) => out.push(c),
            }
        }
    }

    /// Tokenize code inside `${ ... }` up to and including the unmatched `}`.
    fn code_until_close_brace(&mut self, template_start: usize) -> Result<(), ScanError> {
        let mut depth = 0usize;
        let mut prev: Option<Token<'a>> = None;
        loop {
            match self.next_token(prev.as_ref())? {
                None => return err(template_start, "unterminated template substitution"),
                Some(t) => {
                    if t.is("{") {
                        depth += 1;
                    } else if t.is("}") {
                        if depth == 0 {
                            return Ok(());
                        }
                        depth -= 1;
                    }
                    prev = Some(t);
                }
            }
        }
    }

    /// Read a regex literal starting at the `/`.
    fn regex(&mut self) -> Result<(), ScanError> {
        let start = self.pos;
        self.bump();
        let mut in_class = false;
        loop {
            match self.bump() {
                None | Some('\n') | Some('\r') => return err(start, "unterminated regex literal"),
                Some('\\') => {
                    if matches!(self.bump(), None | Some('\n') | Some('\r')) {
                        return err(start, "unterminated regex literal");
                    }
                }
                Some('[') => in_class = true,
                Some(']') => in_class = false,
                Some('/') if !in_class => break,
                Some(_) => {}
            }
        }
        while self.peek().is_some_and(is_ident_char) {
            self.bump();
        }
        Ok(())
    }

    fn number(&mut self) {
        let start = self.pos;
        let hex = self.src[start..].starts_with("0x") || self.src[start..].starts_with("0X");
        let mut prev = ' ';
        while let Some(c) = self.peek() {
            let exp_sign = (c == '+' || c == '-') && (prev == 'e' || prev == 'E') && !hex;
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || exp_sign {
                prev = c;
                self.bump();
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self, prev: Option<&Token<'a>>) -> Result<Option<Token<'a>>, ScanError> {
        self.skip_trivia()?;
        let start = self.pos;
        let c = match self.peek() {
            None => return Ok(None),
            Some(c) => c,
        };
        let kind = if c == '\'' || c == '"' {
            Kind::Str(self.string(c)?)
        } else if c == '`' {
            Kind::Template(self.template()?)
        } else if c.is_ascii_digit() || (c == '.' && self.peek_at(1).is_some_and(|d| d.is_ascii_digit())) {
            self.number();
            Kind::Num
        } else if is_ident_start(c) {
            self.bump();
            while self.peek().is_some_and(is_ident_char) {
                self.bump();
            }
            Kind::Ident
        } else if c == '/' && slash_starts_regex(prev) {
            self.regex()?;
            Kind::Regex
        } else {
            let rest = &self.src[self.pos..];
            match PUNCTS.iter().find(|p| rest.starts_with(**p)) {
                Some(p) => {
                    self.pos += p.len();
                    Kind::Punct
                }
                None => return err(start, format!("unexpected character {:?}", c)),
            }
        };
        Ok(Some(Token { kind, text: &self.src[start..self.pos], start, end: self.pos }))
    }
}

/// Tokenize a whole module.
pub fn tokenize(src: &str) -> Result<Vec<Token<'_>>, ScanError> {
    let mut lx = Lexer { src, pos: 0, depth: 0 };
    let mut out: Vec<Token> = Vec::new();
    while let Some(t) = lx.next_token(out.last())? {
        out.push(t);
    }
    Ok(out)
}

// ============================================================================
// Literal values (scriptlet metadata objects)
// ============================================================================

/// A JavaScript literal as uBO writes scriptlet metadata. Identifiers are kept as names
/// (`fn: someFn`, `dependencies: [ safeSelf ]`) for the caller to resolve.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Num(String),
    Bool(bool),
    Null,
    Ident(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

/// Parse a literal starting at `toks[i]`; returns the value and the index after it.
pub fn parse_value(toks: &[Token], i: usize) -> Result<(Value, usize), ScanError> {
    parse_value_at(toks, i, 0)
}

fn parse_value_at(toks: &[Token], i: usize, depth: usize) -> Result<(Value, usize), ScanError> {
    let t = toks.get(i).ok_or_else(|| ScanError("unexpected end of literal".into()))?;
    if depth > MAX_NESTING {
        return err(t.start, "literal nested too deeply");
    }
    match &t.kind {
        Kind::Str(s) => Ok((Value::Str(s.clone()), i + 1)),
        Kind::Template(Some(s)) => Ok((Value::Str(s.clone()), i + 1)),
        Kind::Num => Ok((Value::Num(t.text.to_string()), i + 1)),
        Kind::Ident => Ok((
            match t.text {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                "null" => Value::Null,
                other => Value::Ident(other.to_string()),
            },
            i + 1,
        )),
        Kind::Punct if t.text == "[" => {
            let mut items = Vec::new();
            let mut j = i + 1;
            loop {
                if toks.get(j).is_some_and(|t| t.is("]")) {
                    return Ok((Value::Array(items), j + 1));
                }
                let (v, next) = parse_value_at(toks, j, depth + 1)?;
                items.push(v);
                j = next;
                match toks.get(j) {
                    Some(t) if t.is(",") => j += 1,
                    Some(t) if t.is("]") => {}
                    _ => return err(toks.get(j).map_or(t.start, |t| t.start), "expected , or ] in array"),
                }
            }
        }
        Kind::Punct if t.text == "{" => {
            let mut fields = Vec::new();
            let mut j = i + 1;
            loop {
                if toks.get(j).is_some_and(|t| t.is("}")) {
                    return Ok((Value::Object(fields), j + 1));
                }
                let key = match toks.get(j) {
                    Some(Token { kind: Kind::Ident, text, .. }) => text.to_string(),
                    Some(Token { kind: Kind::Str(s), .. }) => s.clone(),
                    _ => return err(toks.get(j).map_or(t.start, |t| t.start), "expected a property name"),
                };
                if !toks.get(j + 1).is_some_and(|t| t.is(":")) {
                    return err(toks[j].start, format!("expected ':' after property {:?}", key));
                }
                let (v, next) = parse_value_at(toks, j + 2, depth + 1)?;
                fields.push((key, v));
                j = next;
                match toks.get(j) {
                    Some(t) if t.is(",") => j += 1,
                    Some(t) if t.is("}") => {}
                    _ => return err(toks.get(j).map_or(t.start, |t| t.start), "expected , or } in object"),
                }
            }
        }
        _ => err(t.start, format!("unsupported literal {:?}", t.text)),
    }
}

// ============================================================================
// Top-level module structure
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum Import {
    /// `import './x.js';`
    SideEffect { spec: String },
    /// `import { a, b as c } from './x.js';` — (local name, exported name)
    Named { spec: String, names: Vec<(String, String)> },
}

/// What the scanner found at the top level of one module.
#[derive(Debug, Default)]
pub struct Module {
    /// Imports, in source order.
    pub imports: Vec<Import>,
    /// Top-level `function`/`class` declarations: name -> exact source text.
    pub decls: HashMap<String, String>,
    /// `const X = Y;` where Y is a plain identifier: X -> Y.
    pub aliases: HashMap<String, String>,
    /// Registrations in source order: (`fn` identifier, metadata object fields).
    pub registrations: Vec<(String, Vec<(String, Value)>)>,
}

/// Index of the token after the bracket that closes the one at `toks[i]`.
fn skip_balanced(toks: &[Token], i: usize) -> Result<usize, ScanError> {
    let mut depth = 0usize;
    for (j, t) in toks.iter().enumerate().skip(i) {
        if t.kind == Kind::Punct {
            match t.text {
                "{" | "(" | "[" => depth += 1,
                "}" | ")" | "]" => {
                    depth = depth.checked_sub(1).ok_or_else(|| ScanError(format!("at byte {}: unbalanced {}", t.start, t.text)))?;
                    if depth == 0 {
                        return Ok(j + 1);
                    }
                }
                _ => {}
            }
        }
    }
    err(toks[i].start, format!("unclosed {}", toks[i].text))
}

fn expect<'t>(toks: &'t [Token], i: usize, s: &str) -> Result<&'t Token<'t>, ScanError> {
    match toks.get(i) {
        Some(t) if t.is(s) => Ok(t),
        Some(t) => err(t.start, format!("expected {:?}, found {:?}", s, t.text)),
        None => Err(ScanError(format!("expected {:?}, found end of file", s))),
    }
}

fn ident<'t>(toks: &'t [Token], i: usize) -> Result<&'t str, ScanError> {
    match toks.get(i) {
        Some(Token { kind: Kind::Ident, text, .. }) => Ok(text),
        Some(t) => err(t.start, format!("expected an identifier, found {:?}", t.text)),
        None => Err(ScanError("expected an identifier, found end of file".into())),
    }
}

fn object_fields(v: Value, at: usize) -> Result<Vec<(String, Value)>, ScanError> {
    match v {
        Value::Object(f) => Ok(f),
        _ => err(at, "expected an object literal"),
    }
}

/// Scan one module's top level.
pub fn scan_module(src: &str) -> Result<Module, ScanError> {
    let toks = tokenize(src)?;
    let mut m = Module::default();
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        // Statement-leading `export` is transparent for our purposes.
        let mut k = if t.is("export") { i + 1 } else { i };
        let lead = match toks.get(k) {
            Some(t) => t,
            None => break,
        };

        if lead.is("import") && k == i {
            let spec_tok = toks.get(k + 1).ok_or_else(|| ScanError("import at end of file".into()))?;
            if let Kind::Str(spec) = &spec_tok.kind {
                m.imports.push(Import::SideEffect { spec: spec.clone() });
                i = k + 2;
            } else if spec_tok.is("{") {
                let close = skip_balanced(&toks, k + 1)?;
                let mut names = Vec::new();
                let mut j = k + 2;
                while j < close - 1 {
                    let exported = ident(&toks, j)?.to_string();
                    let mut local = exported.clone();
                    j += 1;
                    if toks[j].is("as") {
                        local = ident(&toks, j + 1)?.to_string();
                        j += 2;
                    }
                    names.push((local, exported));
                    if toks[j].is(",") {
                        j += 1;
                    }
                }
                expect(&toks, close, "from")?;
                let spec = match toks.get(close + 1).map(|t| &t.kind) {
                    Some(Kind::Str(s)) => s.clone(),
                    _ => return err(toks[close].start, "expected a module specifier after from"),
                };
                m.imports.push(Import::Named { spec, names });
                i = close + 2;
            } else {
                return err(spec_tok.start, "unsupported import form (only side-effect and named imports)");
            }
            if toks.get(i).is_some_and(|t| t.is(";")) {
                i += 1;
            }
            continue;
        }

        // Declarations only at the start of a statement: `const x = function() {}` is not one.
        let stmt_start = i == 0 || matches!(toks[i - 1].text, ";" | "}") && toks[i - 1].kind == Kind::Punct;
        if lead.is("async") && toks.get(k + 1).is_some_and(|t| t.is("function")) {
            k += 1;
        }
        if stmt_start && (toks[k].is("function") || toks[k].is("class")) {
            let decl_start = lead.start;
            let is_class = toks[k].is("class");
            let mut j = k + 1;
            if toks.get(j).is_some_and(|t| t.is("*")) {
                j += 1;
            }
            let name = ident(&toks, j)?.to_string();
            j += 1;
            if !is_class {
                expect(&toks, j, "(")?;
                j = skip_balanced(&toks, j)?;
            } else {
                while toks.get(j).is_some_and(|t| !t.is("{")) {
                    j += 1;
                }
            }
            expect(&toks, j, "{")?;
            let end = skip_balanced(&toks, j)?;
            let text = src[decl_start..toks[end - 1].end].to_string();
            if m.decls.insert(name.clone(), text).is_some() {
                return err(decl_start, format!("duplicate top-level declaration {:?}", name));
            }
            i = end;
            continue;
        }

        if (toks[k].is("const") || toks[k].is("let") || toks[k].is("var"))
            && toks.get(k + 2).is_some_and(|t| t.is("="))
            && toks.get(k + 3).is_some_and(|t| t.kind == Kind::Ident)
            && toks.get(k + 4).is_some_and(|t| t.is(";"))
        {
            m.aliases.insert(ident(&toks, k + 1)?.to_string(), toks[k + 3].text.to_string());
            i = k + 5;
            continue;
        }

        // A registration call not at the start of a statement would otherwise be skipped
        // silently, shrinking the set; refuse it.
        let call_like = |j: usize| toks.get(j + 1).is_some_and(|t| t.is("(") || t.is("."));
        if !stmt_start && (toks[k].is("registerScriptlet") || toks[k].is("builtinScriptlets")) && call_like(k) {
            return err(toks[k].start, format!("{} used outside a statement start", toks[k].text));
        }

        if stmt_start && toks[k].is("registerScriptlet") && k == i {
            expect(&toks, k + 1, "(")?;
            let fn_name = ident(&toks, k + 2)?.to_string();
            expect(&toks, k + 3, ",")?;
            let (v, next) = parse_value(&toks, k + 4)?;
            let fields = object_fields(v, toks[k + 4].start)?;
            expect(&toks, next, ")")?;
            m.registrations.push((fn_name, fields));
            i = next + 1;
            continue;
        }

        if stmt_start && toks[k].is("builtinScriptlets") && k == i && toks.get(k + 1).is_some_and(|t| t.is(".")) {
            expect(&toks, k + 2, "push")?;
            expect(&toks, k + 3, "(")?;
            let mut j = k + 4;
            loop {
                let (v, next) = parse_value(&toks, j)?;
                let fields = object_fields(v, toks[j].start)?;
                let fn_name = match fields.iter().find(|(key, _)| key == "fn") {
                    Some((_, Value::Ident(n))) => n.clone(),
                    _ => return err(toks[j].start, "builtinScriptlets.push entry without an identifier `fn`"),
                };
                m.registrations.push((fn_name, fields));
                if toks.get(next).is_some_and(|t| t.is(",")) {
                    j = next + 1;
                } else {
                    expect(&toks, next, ")")?;
                    i = next + 1;
                    break;
                }
            }
            continue;
        }

        if t.is("export") && toks[k].is("{") {
            // `export { a, b }` (local re-export) is harmless; `export ... from` is not handled.
            let close = skip_balanced(&toks, k)?;
            if toks.get(close).is_some_and(|t| t.is("from")) {
                return err(toks[k].start, "re-export `export { } from` is not supported");
            }
            i = close;
            continue;
        }
        if t.is("export") && (toks[k].is("default") || toks[k].is("*")) {
            return err(t.start, "default / star exports are not supported");
        }

        // Any other statement: step over it, keeping brackets balanced.
        if t.kind == Kind::Punct && matches!(t.text, "{" | "(" | "[") {
            i = skip_balanced(&toks, i)?;
        } else if t.kind == Kind::Punct && matches!(t.text, "}" | ")" | "]") {
            return err(t.start, format!("unbalanced {}", t.text));
        } else {
            i += 1;
        }
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<(Kind, String)> {
        tokenize(src).unwrap().into_iter().map(|t| (t.kind, t.text.to_string())).collect()
    }

    #[test]
    fn regex_vs_division() {
        // After a value it is division; after an operator or keyword it is a regex.
        for src in ["a / b / c", "(a + b) / 2 / c", "x[0] / y / z", "f() / '}' / g", "1 / 2 / 3"] {
            let k = kinds(src);
            assert_eq!(k.iter().filter(|(k, _)| *k == Kind::Regex).count(), 0, "division misread as regex: {}", src);
        }
        let k = kinds("x = /a}b/g; return /[/]/.test(y); f(i++ / 2)");
        let regexes: Vec<_> = k.iter().filter(|(k, _)| *k == Kind::Regex).map(|(_, t)| t.as_str()).collect();
        assert_eq!(regexes, vec!["/a}b/g", "/[/]/"]);
        // After a block's closing brace a new statement starts: a regex, not division.
        let m = scan_module("function f(a) { if (a) { a(); }
/[}]/.test(a); }
function g() {}").unwrap();
        assert_eq!(m.decls["f"], "function f(a) { if (a) { a(); }
/[}]/.test(a); }");
        // A regex containing an escaped slash and a brace does not end early.
        let k = kinds(r"if (s) { y = s.replace(/\/{/g, '') }");
        assert!(k.iter().any(|(k, t)| *k == Kind::Regex && t == r"/\/{/g"));
    }

    #[test]
    fn strings_and_comments_hide_braces() {
        let m = scan_module(
            "function a() { const s = '}'; const d = \"{\"; /* } */ // }\n return 1; }\nfunction b() {}",
        ).unwrap();
        assert_eq!(m.decls["a"], "function a() { const s = '}'; const d = \"{\"; /* } */ // }\n return 1; }");
        assert_eq!(m.decls["b"], "function b() {}");
    }

    #[test]
    fn template_literals_nest() {
        let src = "function t(x) { return `a}${ x ? `in}${ {a:1}.a }` : '}' }b{`; }\nfunction u() {}";
        let m = scan_module(src).unwrap();
        assert_eq!(m.decls["t"], "function t(x) { return `a}${ x ? `in}${ {a:1}.a }` : '}' }b{`; }");
        assert!(m.decls.contains_key("u"));
        // A regex inside a substitution is lexed as a regex, not as division.
        let m = scan_module("function v(s) { return `${ s.replace(/}/g, '') }`; }").unwrap();
        assert!(m.decls.contains_key("v"));
    }

    #[test]
    fn string_escapes_decode() {
        let k = kinds(r#"'a\'b' "c\"d" 'e\x41B\u{43}' 'f\
g'"#);
        let strs: Vec<_> = k.into_iter().filter_map(|(k, _)| if let Kind::Str(s) = k { Some(s) } else { None }).collect();
        assert_eq!(strs, vec!["a'b", "c\"d", "eABC", "fg"]);
    }

    #[test]
    fn unterminated_inputs_are_errors() {
        for src in ["'abc", "`abc", "x = /abc", "/* abc", "`${ a `"] {
            assert!(tokenize(src).is_err(), "should fail: {}", src);
        }
        assert!(scan_module("function a() {").is_err());
    }

    #[test]
    fn declarations_and_exports() {
        let m = scan_module(
            "export function a(x) { return { b: 1 }; }\nasync function c() {}\nfunction* g() {}\n\
             export class K extends Base { m() { return '}'; } }\nexport const J = __J__;\n\
             const notAlias = [];\nexport { a };",
        ).unwrap();
        assert_eq!(m.decls["a"], "function a(x) { return { b: 1 }; }");
        assert_eq!(m.decls["c"], "async function c() {}");
        assert_eq!(m.decls["g"], "function* g() {}");
        assert_eq!(m.decls["K"], "class K extends Base { m() { return '}'; } }");
        assert_eq!(m.aliases["J"], "__J__");
        assert!(!m.aliases.contains_key("notAlias"));
    }

    #[test]
    fn imports_parse() {
        let m = scan_module("import './a.js';\nimport { x, y as z } from './b.js';\nimport {\n w,\n} from '../c.js'").unwrap();
        assert_eq!(m.imports, vec![
            Import::SideEffect { spec: "./a.js".into() },
            Import::Named { spec: "./b.js".into(), names: vec![("x".into(), "x".into()), ("z".into(), "y".into())] },
            Import::Named { spec: "../c.js".into(), names: vec![("w".into(), "w".into())] },
        ]);
        assert!(scan_module("import d from './d.js';").is_err());
        assert!(scan_module("import * as ns from './d.js';").is_err());
    }

    #[test]
    fn registrations_both_shapes() {
        let m = scan_module(
            "registerScriptlet(fooFn, {\n name: 'foo.fn',\n dependencies: [ safeSelf, 'bar.fn' ],\n});\n\
             builtinScriptlets.push({\n name: 'x.js',\n aliases: [ 'y.js', ],\n fn: xFn,\n requiresTrust: true,\n world: 'ISOLATED',\n}, { name: 'z.js', fn: zFn });",
        ).unwrap();
        assert_eq!(m.registrations.len(), 3);
        assert_eq!(m.registrations[0].0, "fooFn");
        assert_eq!(m.registrations[0].1[1], ("dependencies".into(),
            Value::Array(vec![Value::Ident("safeSelf".into()), Value::Str("bar.fn".into())])));
        assert_eq!(m.registrations[1].0, "xFn");
        assert!(m.registrations[1].1.contains(&("requiresTrust".into(), Value::Bool(true))));
        assert_eq!(m.registrations[2].0, "zFn");
    }

    #[test]
    fn unrecognised_registration_shapes_fail_loudly() {
        // A registration we cannot read must not be silently skipped.
        assert!(scan_module("registerScriptlet(fooFn, details);").is_err());
        assert!(scan_module("registerScriptlet(fooFn, { ...base });").is_err());
        assert!(scan_module("builtinScriptlets.push({ name: 'a.js', fn: () => {} });").is_err());
        assert!(scan_module("builtinScriptlets.push(...list);").is_err());
        assert!(scan_module("x = registerScriptlet(a, { name: 'a.fn' });").is_err());
        assert!(scan_module("f(), builtinScriptlets.push({ name: 'a.js', fn: a });").is_err());
        // ...but defining the helper itself is fine (uBO's base.js).
        let m = scan_module("export const registerScriptlet = (fn, details) => { registeredScriptlets.push(details); };").unwrap();
        assert!(m.registrations.is_empty());
    }

    #[test]
    fn hostile_nesting_is_an_error_not_a_stack_overflow() {
        let deep_template = format!("x = {}1{};", "`${".repeat(10_000), "}`".repeat(10_000));
        assert!(tokenize(&deep_template).is_err());
        let deep_literal = format!("registerScriptlet(f, {{ name: {}1{} }});", "[".repeat(10_000), "]".repeat(10_000));
        assert!(scan_module(&deep_literal).is_err());
        // Ordinary nesting is fine.
        assert!(tokenize("x = `a${ `b${ `c${ 1 }` }` }`;").is_ok());
    }

    #[test]
    fn function_expressions_are_not_declarations() {
        let m = scan_module("const x = function() { return '}'; };\nconst y = [function named() {}];\nfunction z() {}").unwrap();
        assert_eq!(m.decls.keys().collect::<Vec<_>>(), vec!["z"]);
    }

    #[test]
    fn numbers_with_exponents_do_not_swallow_operators() {
        let k = kinds("a = 1e-5 + 0x1e + 2");
        let nums: Vec<_> = k.iter().filter(|(k, _)| *k == Kind::Num).map(|(_, t)| t.as_str()).collect();
        assert_eq!(nums, vec!["1e-5", "0x1e", "2"]);
    }

    #[test]
    fn nested_registration_calls_are_not_top_level() {
        let m = scan_module("const f = () => { registerScriptlet(a, b); };\nfunction g() { builtinScriptlets.push(x); }").unwrap();
        assert!(m.registrations.is_empty());
        assert!(m.decls.contains_key("g"));
    }
}
