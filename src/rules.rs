use std::sync::LazyLock;

use regex::Regex;

use crate::Diagnostic;

const ORDERED_LIST: &str = "deordinal/ordered-list";
const ORDINAL_PREFIX: &str = "deordinal/ordinal-prefix";
const KEYWORD_PREFIX: &str = "deordinal/keyword-prefix";

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
    Diagnostic::warning(ORDERED_LIST, "番号付きリストを使用しています", start, end)
}

pub(crate) fn check_line(line: &str, start: usize, diagnostics: &mut Vec<Diagnostic>) {
    let text = line.trim_start_matches([' ', '\t']);
    let start = start + line.len() - text.len();
    let found = if let Some(label) = KEYWORD.find(text) {
        Some((KEYWORD_PREFIX, "先頭に段階ラベルがあります", label.end()))
    } else if let Some(label) = BRACKETED.find(text) {
        Some((ORDINAL_PREFIX, "先頭に順序ラベルがあります", label.end()))
    } else if let Some(label) = NUMBER.captures(text).filter(|m| numbered_label(text, m)) {
        Some((
            ORDINAL_PREFIX,
            "先頭に順序ラベルがあります",
            label.get(0).unwrap().end(),
        ))
    } else {
        LETTER
            .find(text)
            .or_else(|| CIRCLED.find(text))
            .map(|label| (ORDINAL_PREFIX, "先頭に順序ラベルがあります", label.end()))
    };
    if let Some((rule, message, len)) = found {
        let end = start + text[..len].trim_end_matches([' ', '\t']).len();
        diagnostics.push(Diagnostic::warning(rule, message, start, end));
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
        let unit = token.split('/').next().unwrap_or(token);
        const UNITS: &[&str] = &[
            "GB", "MB", "KB", "TB", "GHz", "MHz", "Hz", "ms", "px", "mm", "cm", "km", "m", "kg",
            "g", "BTC", "ETH", "USDC",
        ];
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
        if UNITS.contains(&unit)
            || JAPANESE_FOLLOWERS
                .iter()
                .any(|word| token.starts_with(word))
            || rest.starts_with(['=', '<', '>', '±'])
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(text: &str) -> Option<&'static str> {
        let mut diagnostics = Vec::new();
        check_line(text, 0, &mut diagnostics);
        diagnostics.first().map(|d| d.rule)
    }

    #[test]
    fn ordinal_examples() {
        for case in [
            "1. 概要",
            "1: 設定",
            "1) 概要",
            "1．導入",
            "1、準備",
            "1.29 機能説明",
            "1.2.3 設計",
            "1.29: 詳細",
            "A) 手順",
            "a. 手順",
            "(1) 概要",
            "[a] 概要",
            "(一) 概要",
            "① 項目",
            "㊿ 項目",
            "一、 はじめに",
        ] {
            assert_eq!(rule(case), Some(ORDINAL_PREFIX), "{case}");
        }
    }

    #[test]
    fn keyword_examples() {
        for case in [
            "Step 1",
            "Step 1: 初期化",
            "Phase A",
            "phase A: 計画",
            "ステップ 1",
            "フェーズ1: 開発",
            "タスク 1. 調査",
            "ステージ ① 準備",
        ] {
            assert_eq!(rule(case), Some(KEYWORD_PREFIX), "{case}");
        }
    }

    #[test]
    fn negative_examples() {
        for case in [
            "e.g. 例えば",
            "i.e., つまり",
            "https://example.com/step1",
            "1.29 GB",
            "1.29 GB/s の転送",
            "1.29 は 1.3 未満",
            "1.0.0 のリリース",
            "1.29 以下の場合",
            "1.29 = x",
            "2025 年の目標",
            "7 倍の高速化",
            "7倍の高速化",
            "100 円のコスト",
            "1 BTC",
            "- 項目",
            "空の見出し",
            "",
            "   ",
            "Step 1abc",
            "API. documentation",
            "㋐ 項目",
        ] {
            assert_eq!(rule(case), None, "{case}");
        }
    }

    #[test]
    fn diagnostics_point_at_label() {
        let mut diagnostics = Vec::new();
        check_line("  Step 1: 準備", 10, &mut diagnostics);
        assert_eq!(diagnostics[0].start, 12);
        assert_eq!(diagnostics[0].rule, KEYWORD_PREFIX);
    }
}
