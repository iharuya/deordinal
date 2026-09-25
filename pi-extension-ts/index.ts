import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { checkContent, isTargetFile } from "./checker.ts";
import type { Violation } from "./types.ts";

export * from "./checker.ts";
export * from "./comment.ts";
export * from "./pattern.ts";
export * from "./types.ts";

function formatViolationList(violations: Violation[]): string {
	const details = violations
		.slice(0, 5)
		.map((v) => `  - 行 ${v.line}: "${v.text}" (${v.reason})`)
		.join("\n");
	const more =
		violations.length > 5 ? `\n  ... 他 ${violations.length - 5} 件` : "";
	return `${details}${more}`;
}

function formatBlockMessage(violations: Violation[]): string {
	return (
		`🚫 ドキュメントおよびコードコメント内での番号振りは禁止されています。\n` +
		`コードやドキュメントの更新時に番号がずれてメンテナンスコストが高くなることを防ぐ目的です。\n\n` +
		`違反箇所:\n${formatViolationList(violations)}\n\n` +
		`修正方法:\n` +
		`- 見出しやコードコメントから番号・ステップ表記（"step 1:", "phase A:", "1. ", "a. " など）を削除し、役割・処理内容のみを記述してください。\n` +
		`- 箇条書きには番号付きリストを使わず、順序なし箇条書きリスト（"- "）を使用してください。\n` +
		`もしこの目的と無関係に書き込みがブロックされてしまった場合は、回避するのではなく、立ち止まってユーザーに確認してください。`
	);
}

function isTargetTool(
	toolName: string,
): toolName is "write" | "edit" | `${string}:write` | `${string}:edit` {
	return (
		toolName === "write" ||
		toolName.endsWith(":write") ||
		toolName === "edit" ||
		toolName.endsWith(":edit")
	);
}

type ToolCallInput = {
	path?: string;
	content?: string;
	edits?: Array<{ newText?: string }>;
};

function collectViolations(
	toolName: string,
	input: ToolCallInput,
): Violation[] {
	const filePath = input.path || "";
	if (!isTargetFile(filePath)) return [];

	if (toolName === "write" || toolName.endsWith(":write")) {
		return checkContent(input.content || "", filePath);
	}

	if (toolName === "edit" || toolName.endsWith(":edit")) {
		const edits = input.edits || [];
		const violations: Violation[] = [];
		for (const edit of edits) {
			if (typeof edit?.newText === "string") {
				violations.push(...checkContent(edit.newText, filePath));
			}
		}
		return violations;
	}

	return [];
}

export default function (pi: ExtensionAPI) {
	pi.on("tool_call", async (event, ctx) => {
		const toolName = event.toolName.toLowerCase();
		if (!isTargetTool(toolName)) return;

		const input = event.input as ToolCallInput | undefined;
		if (!input) return;

		const violations = collectViolations(toolName, input);
		if (violations.length === 0) return;

		if (ctx.hasUI) {
			ctx.ui.notify("ナンバリングをブロックしました", "warning");
		}

		return {
			block: true,
			reason: formatBlockMessage(violations),
		};
	});
}
