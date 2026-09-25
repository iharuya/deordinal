function isShebang(line: string): boolean {
	return /^#!/.test(line);
}

function isCppDirective(line: string): boolean {
	return /^#\s*(?:include|define|undef|if|ifdef|ifndef|elif|else|endif|pragma|error|warning)\b/.test(
		line,
	);
}

function isCompilerOrLinterDirective(line: string): boolean {
	return /^\/\/\s*@ts-|^#\s*type:\s*ignore|^\/\*\s*eslint-|^\/\/\s*eslint-/.test(
		line,
	);
}

function matchSlashComment(line: string): string | null {
	const match = line.match(/^\/\/\s*(.*)$/);
	return match ? match[1] : null;
}

function matchHashComment(line: string): string | null {
	const match = line.match(/^#\s*(.*)$/);
	return match ? match[1] : null;
}

function matchDashComment(line: string): string | null {
	const match = line.match(/^--\s*(.*)$/);
	return match ? match[1] : null;
}

function matchBlockComment(line: string): string | null {
	const inline = line.match(/^\/\*\s*(.*?)\s*\*\/$/);
	if (inline) return inline[1];

	const start = line.match(/^\/\*\s*(.*)$/);
	if (start) return start[1];

	const continuation = line.match(/^\*\s*(.*)$/);
	if (continuation) return continuation[1];

	return null;
}

export function extractCommentContent(line: string): string | null {
	const trimmed = line.trim();
	if (
		isShebang(trimmed) ||
		isCppDirective(trimmed) ||
		isCompilerOrLinterDirective(trimmed)
	) {
		return null;
	}

	return (
		matchSlashComment(trimmed) ??
		matchHashComment(trimmed) ??
		matchDashComment(trimmed) ??
		matchBlockComment(trimmed)
	);
}
