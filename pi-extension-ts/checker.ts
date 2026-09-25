import { extractCommentContent } from "./comment.ts";
import { checkNumberedPattern } from "./pattern.ts";
import type { Violation } from "./types.ts";

const DOC_EXTENSIONS = /\.(md|markdown|mdx|txt|rst|adoc)$/i;
const CODE_EXTENSIONS =
	/\.(ts|tsx|js|jsx|mjs|cjs|cts|mts|vue|svelte|py|pyw|ipynb|go|rs|c|cpp|cc|cxx|h|hpp|hxx|java|kt|kts|scala|cs|fs|vb|rb|erb|rake|php|sh|bash|zsh|fish|sql|swift|dart|lua|yaml|yml|toml|tf|hcl)$/i;

export function isTargetFile(filePath: string): boolean {
	const basename = filePath.split("/").pop() || "";
	if (/^(Dockerfile|Makefile|Containerfile)$/i.test(basename)) return true;
	return DOC_EXTENSIONS.test(filePath) || CODE_EXTENSIONS.test(filePath);
}

export function isDocFile(filePath: string): boolean {
	return DOC_EXTENSIONS.test(filePath);
}

function isFrontmatterDelimiter(line: string): boolean {
	return /^(?:---|\+\+\+)\s*$/.test(line);
}

function isTableLine(line: string): boolean {
	return /^\|.*\|\s*$/.test(line);
}

function isReferenceOrFootnote(line: string): boolean {
	return /^\[\^?[^\]]+\]:\s+/.test(line);
}

function isHorizontalRule(line: string): boolean {
	return /^(?:-{3,}|\*{3,}|_{3,})\s*$/.test(line);
}

function stripBlockquotePrefix(line: string): string {
	return line.replace(/^(\s*>\s*)+/, "").trim();
}

function extractMarkdownHeading(line: string): string | null {
	const match = line.match(/^(#{1,6})\s+(.*)$/);
	return match ? match[2].trim() : null;
}

function extractTaskListItem(line: string): string | null {
	const match = line.match(/^[-*+]\s+\[([ xX])\]\s+(.*)$/);
	return match ? match[2].trim() : null;
}

function extractBulletListItem(line: string): string | null {
	const match = line.match(/^[-*+]\s+(.*)$/);
	return match ? match[1].trim() : null;
}

function checkDocumentLine(line: string): string | null {
	const stripped = stripBlockquotePrefix(line);
	if (!stripped) return null;

	const targetText =
		extractMarkdownHeading(stripped) ??
		extractTaskListItem(stripped) ??
		extractBulletListItem(stripped) ??
		stripped;

	return checkNumberedPattern(targetText);
}

function checkCodeLine(line: string): string | null {
	const commentBody = extractCommentContent(line);
	return commentBody !== null ? checkNumberedPattern(commentBody) : null;
}

export function checkContent(content: string, filePath: string): Violation[] {
	const lines = content.split(/\r?\n/);
	const violations: Violation[] = [];
	const isDoc = isDocFile(filePath);

	let inFencedCodeBlock = false;
	let codeFenceChar = "";
	let inFrontmatter = false;
	let inHtmlComment = false;

	for (let i = 0; i < lines.length; i++) {
		const line = lines[i];
		const trimmed = line.trim();

		if (!trimmed) continue;

		if (isDoc) {
			if (i === 0 && isFrontmatterDelimiter(trimmed)) {
				inFrontmatter = true;
				continue;
			}
			if (inFrontmatter) {
				if (isFrontmatterDelimiter(trimmed)) {
					inFrontmatter = false;
				}
				continue;
			}

			const fenceMatch = line.match(/^(\s*)(`{3,}|~{3,})/);
			if (fenceMatch) {
				const fence = fenceMatch[2];
				if (!inFencedCodeBlock) {
					inFencedCodeBlock = true;
					codeFenceChar = fence[0];
				} else if (fence.startsWith(codeFenceChar)) {
					inFencedCodeBlock = false;
					codeFenceChar = "";
				}
				continue;
			}
			if (inFencedCodeBlock) continue;

			if (!inHtmlComment && /<!--/.test(line)) {
				if (!/-->/.test(line)) {
					inHtmlComment = true;
				}
				continue;
			}
			if (inHtmlComment) {
				if (/-->/.test(line)) {
					inHtmlComment = false;
				}
				continue;
			}

			if (
				isTableLine(trimmed) ||
				isReferenceOrFootnote(trimmed) ||
				isHorizontalRule(trimmed)
			) {
				continue;
			}

			const reason = checkDocumentLine(trimmed);
			if (reason) {
				violations.push({
					line: i + 1,
					text: trimmed,
					type: "doc",
					reason,
				});
			}
		} else {
			const reason = checkCodeLine(line);
			if (reason) {
				violations.push({
					line: i + 1,
					text: trimmed,
					type: "code_comment",
					reason,
				});
			}
		}
	}

	return violations;
}
