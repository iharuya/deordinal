const KEYWORD_PREFIXES =
	"(?:step|phase|part|stage|section|task|ステップ|フェーズ|フェイズ|パート|ステージ|セクション|タスク)";

const KEYWORD_REGEX = new RegExp(
	`^${KEYWORD_PREFIXES}\\s*(?:[0-9]+|[a-zA-Z]|[ivxIVX]+|[①-⑳]|[一二三四五六七八九十]+)[\\:\\.\\s\\-、．\\)]`,
	"i",
);

const UNIT_PATTERN =
	/^(?:(?:AAVE|ADA|ATOM|AVAX|BCH|BNB|BTC|DAI|DOGE|DOT|ETH|LINK|LTC|NEAR|SHIB|SOL|SUI|TON|TRX|UNI|USDC|USDT|XLM|XMR|XRP|ZEC|[kKMGTPEkmgtpe]?i?[Bb](?:ps|it)?|[kKMGTPEkmgtpe]?(?:Hz|hz|s|sec|secs|second|seconds|min|mins|minute|minutes|h|hr|hrs|hour|hours|day|days|yr|yrs|year|years|m|meter|meters|g|gram|grams|t|ton|tons|l|L|liter|liters|N|Pa|hPa|kPa|MPa|bar|atm|cal|kcal|J|kJ|W|kW|MW|GW|mW|V|mV|kV|A|mA|uA|µA|deg|rad|K|px|pt|pc|em|rem|ex|ch|vw|vh|vmin|vmax|dpi|ppi|fps|dB|dBm|in|inch|inches|ft|feet|yd|mi|oz|lb|lbs))(?:\/[a-zA-Z]+)?\b|[%％℃℉°xX倍割分厘円ドルセントユーロポンドウォン万億兆個つ本枚台冊件行文字語点回度人名匹頭羽足着杯通組勝敗階段巡周曲話篇編章節項条])/;

const MULTI_CHAR_MODIFIERS =
	/^(?:という|といった|とする|として|とは|について|に対して|によって|において|および|または|などの|など|等の|等|あたり|毎に|毎|ずつ|につき|以下|以上|未満|超え|超|以内|以降|以前|前後|程度|くらい|位|ほど|近く|付近|余り|あまり|強|弱)/;

const PARTICLES_WITH_DELIMITER =
	/^(?:の|は|が|を|に|へ|で|と|から|より|まで)(?:[\s\u3000、，,]|$|\d)|^の|^から[^\u3040-\u309F]|^より[^\u3040-\u309F]|^まで[^\u3040-\u309F]/;

const MATH_OPERATORS = /^[=≠<>≤≥~≈±+\-*/÷×^]/;

function isLatinAbbreviation(text: string): boolean {
	return /^(?:e\.g\.|i\.e\.)[\s,]/i.test(text);
}

function isUrl(text: string): boolean {
	return /^https?:\/\//i.test(text);
}

function isKeywordPrefixed(text: string): boolean {
	return KEYWORD_REGEX.test(text);
}

function isExplicitDelimiterNumbered(text: string): boolean {
	return (
		/^\d+(?:\.\d+)*\.\s+/.test(text) ||
		/^\d+(?:\.\d+)*\s*[):：、．]\s*/.test(text) ||
		/^\d+(?:\.\d+)*\s+[-–—]\s+/.test(text)
	);
}

function isAlphabetNumbered(text: string): boolean {
	return /^[a-zA-Z][.):、．]\s+/.test(text);
}

function isParenthesizedOrBracketedNumbered(text: string): boolean {
	return (
		/^\(\s*(?:\d+|[a-zA-Z]|[ivxIVX]+|[一二三四五六七八九十]+)\s*\)\s*/.test(
			text,
		) || /^\[\s*(?:\d+|[a-zA-Z])\s*\]\s*/.test(text)
	);
}

function isCircledNumbered(text: string): boolean {
	return /^[①-⑳]\s*/.test(text);
}

function isKanjiNumbered(text: string): boolean {
	return /^[一二三四五六七八九十]+[、､．.]\s*/.test(text);
}

function isPlainNumberedText(text: string): boolean {
	const match = text.match(/^(\d+(?:\.\d+)*)\s+(.+)$/);
	if (!match) return false;

	const [, num, rest] = match;

	if (/^\d{4,}$/.test(num)) return false;

	if (
		UNIT_PATTERN.test(rest) ||
		MULTI_CHAR_MODIFIERS.test(rest) ||
		PARTICLES_WITH_DELIMITER.test(rest) ||
		MATH_OPERATORS.test(rest)
	) {
		return false;
	}

	return true;
}

export function checkNumberedPattern(text: string): string | null {
	const trimmed = text.trim();
	if (!trimmed) return null;
	if (isLatinAbbreviation(trimmed) || isUrl(trimmed)) return null;

	if (isKeywordPrefixed(trimmed)) {
		return "番号振りプレフィックス（step/phase/フェーズ/タスク等）が使われています";
	}
	if (isExplicitDelimiterNumbered(trimmed)) {
		return "番号振りが使われています（例: 1. または 1)）";
	}
	if (isAlphabetNumbered(trimmed)) {
		return "アルファベット番号振りが使われています（例: a. または A)）";
	}
	if (isParenthesizedOrBracketedNumbered(trimmed)) {
		return "括弧付き番号が使われています（例: (1) または [1]）";
	}
	if (isCircledNumbered(trimmed)) {
		return "丸数字が使われています（例: ①）";
	}
	if (isKanjiNumbered(trimmed)) {
		return "漢数字番号が使われています（例: 一、）";
	}
	if (isPlainNumberedText(trimmed)) {
		return "番号振りが使われています（例: 1 はじめに または 1.1 詳細）";
	}

	return null;
}
