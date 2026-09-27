use std::sync::LazyLock;

use regex::Regex;

use crate::Diagnostic;

const ORDERED_LIST: &str = "ordered-list";
const PREFIX: &str = "prefix";
const KEYWORD_PREFIX: &str = "keyword-prefix";

const KEYWORDS: &[&str] = &[
    "step",
    "phase",
    "stage",
    "task",
    "ステップ",
    "フェーズ",
    "ステージ",
    "タスク",
];

static KEYWORD: LazyLock<Regex> = LazyLock::new(|| {
    let alternatives = KEYWORDS
        .iter()
        .map(|word| regex::escape(word))
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(r"(?i)^(?:{alternatives})[ \t]*(?:[0-9]+|[A-Z]|[①-⑳㉑-㉟㊱-㊿])(?:[.):：、．]|[ \t]*[-–—][ \t]*|[ \t]+|$)")).unwrap()
});
static BRACKETED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:\((?:[0-9]+|[A-Za-z]|[一二三四五六七八九十]+)\)|\[(?:[0-9]+|[A-Za-z]|[一二三四五六七八九十]+)\])(?:[ \t]+|$)").unwrap()
});
static NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<number>[0-9]+(?:\.[0-9]+)*)(?P<separator>[.):](?:[ \t]+|$)|[：、．]|[ \t]+(?:[-–—][ \t]+)?)").unwrap()
});
static LETTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[A-Za-z]|[一二三四五六七八九十]+)(?:[.):](?:[ \t]+|$)|[：、．])").unwrap()
});
static CIRCLED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[①-⑳㉑-㉟㊱-㊿](?:[ \t]+|$)").unwrap());

pub(crate) fn ordered_list(start: usize, end: usize) -> Diagnostic {
    Diagnostic::warning(ORDERED_LIST, "Ordered list used", start, end)
}

pub(crate) fn check_line(
    line: &str,
    start: usize,
    diagnostics: &mut Vec<Diagnostic>,
    allow_fix: bool,
) {
    let text = line.trim_start_matches([' ', '\t']);
    let start = start + line.len() - text.len();
    let found = if let Some(label) = KEYWORD.find(text) {
        Some((KEYWORD_PREFIX, "Leading phase label", label.end()))
    } else if let Some(label) = BRACKETED.find(text) {
        Some((PREFIX, "Leading ordering label", label.end()))
    } else if let Some(label) = NUMBER.captures(text).filter(|m| numbered_label(text, m)) {
        Some((
            PREFIX,
            "Leading ordering label",
            label.get(0).unwrap().end(),
        ))
    } else {
        LETTER
            .find(text)
            .or_else(|| CIRCLED.find(text))
            .map(|label| (PREFIX, "Leading ordering label", label.end()))
    };
    if let Some((rule, message, len)) = found {
        let end = start + text[..len].trim_end_matches([' ', '\t']).len();
        let mut diagnostic = Diagnostic::warning(rule, message, start, end);
        let rest = &text[len..];
        let whitespace = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        if allow_fix && rest[whitespace..].chars().any(char::is_alphanumeric) {
            diagnostic.fix = Some(start..start + len + whitespace);
        }
        diagnostics.push(diagnostic);
    }
}

fn numbered_label(text: &str, label: &regex::Captures<'_>) -> bool {
    let number = &label["number"];
    let separator = &label["separator"];
    if separator.starts_with([' ', '\t']) && !number.contains('.') {
        return false;
    }
    if number.contains('.') && separator.trim().is_empty() {
        let rest = text[label.get(0).unwrap().end()..].trim_start();
        let token = rest
            .split(|c: char| c.is_whitespace() || ",，。;；".contains(c))
            .next()
            .unwrap_or("");
        const JAPANESE_FOLLOWERS: &[&str] = &[
            "%",
            "℃",
            "倍",
            "円",
            "人",
            "の",
            "は",
            "を",
            "に",
            "へ",
            "で",
            "と",
            "から",
            "まで",
            "という",
            "など",
            "以上",
            "以下",
            "未満",
            "前後",
            "程度",
        ];
        if is_quantity_unit(token)
            || JAPANESE_FOLLOWERS
                .iter()
                .any(|word| token.starts_with(word))
            || rest.starts_with(['=', '<', '>', '±', '→'])
        {
            return false;
        }
    }
    true
}

fn is_quantity_unit(token: &str) -> bool {
    let unit = token
        .split('/')
        .next()
        .unwrap_or(token)
        .trim_end_matches(['.', ':', '!', '?', ')', ']', '。', '：', '！', '？', '）']);
    const UNITS: &[&str] = &[
        "GB", "MB", "KB", "TB", "GHz", "MHz", "Hz", "ms", "px", "mm", "cm", "km", "m", "kg", "g",
        "BTC", "ETH", "USDC", "s", "sec", "secs", "second", "seconds", "min", "mins", "minute",
        "minutes", "h", "hr", "hrs", "hour", "hours", "day", "days", "week", "weeks", "秒", "分",
        "分間", "時間", "日", "日間", "週間",
    ];
    UNITS.iter().any(|word| unit.eq_ignore_ascii_case(word)) || unit.starts_with('秒')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(text: &str) -> Option<&'static str> {
        let mut diagnostics = Vec::new();
        check_line(text, 0, &mut diagnostics, false);
        diagnostics.first().map(|d| d.rule)
    }

    #[test]
    fn ordinal_examples() {
        for case in [
            "1. Overview",
            "1: Setup",
            "1) Overview",
            "1．Introduction",
            "1、Prepare",
            "1.29 Features",
            "1.2.3 Design",
            "1.29: Details",
            "A) Procedure",
            "a. Procedure",
            "(1) Overview",
            "[a] Overview",
            "(一) Overview",
            "① Item",
            "㊿ Item",
            "一、 Introduction",
        ] {
            assert_eq!(rule(case), Some(PREFIX), "{case}");
        }
    }

    #[test]
    fn keyword_examples() {
        for case in [
            "Step 1",
            "Step 1: Initialize",
            "Phase A",
            "phase A: Plan",
            "ステップ 1",
            "フェーズ1: Develop",
            "タスク 1. Investigate",
            "ステージ ① Prepare",
        ] {
            assert_eq!(rule(case), Some(KEYWORD_PREFIX), "{case}");
        }
    }

    #[test]
    fn negative_examples() {
        for case in [
            "e.g. example",
            "i.e., in other words",
            "https://example.com/step1",
            "1.29 GB",
            "1.29 GB/s transfer",
            "2.5 seconds elapsed",
            "2.5 seconds.",
            "2.5 Sec elapsed",
            "2.5 s elapsed",
            "2.5 hours elapsed",
            "2.5 minutes elapsed",
            "2.5 秒経過",
            "2.5 秒",
            "2.5 時間",
            "2.5 分間",
            "1.29 は 1.3 未満",
            "1.0.0 のリリース",
            "1.29 以下の場合",
            "1.29 = x",
            "0.934 → 0.744",
            "2025 goals",
            "7 倍の高速化",
            "7倍の高速化",
            "100 yen cost",
            "1 BTC",
            "- Item",
            "Plain heading",
            "",
            "   ",
            "Step 1abc",
            "API. documentation",
            "㋐ Item",
        ] {
            assert_eq!(rule(case), None, "{case}");
        }
    }

    #[test]
    fn diagnostics_point_at_label() {
        let mut diagnostics = Vec::new();
        check_line("  Step 1: Prepare", 10, &mut diagnostics, false);
        assert_eq!(diagnostics[0].start, 12);
        assert_eq!(diagnostics[0].rule, KEYWORD_PREFIX);
    }
}
