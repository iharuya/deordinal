import type {
	ExtensionAPI,
	ExtensionContext,
} from "@earendil-works/pi-coding-agent";
import { describe, expect, it, vi } from "vitest";
import registerExtension from "./index.ts";

describe("enforce-no-numbering integration", () => {
	const mockContext = {
		hasUI: true,
		ui: {
			notify: vi.fn(),
		},
	} as unknown as ExtensionContext;

	function setupExtension() {
		let toolCallHandler:
			| ((
					event: { toolName: string; input?: unknown },
					ctx: ExtensionContext,
			  ) => Promise<{ block?: boolean; reason?: string } | undefined>)
			| null = null;

		const mockPi = {
			on: vi.fn(
				(
					event: string,
					handler: (
						event: { toolName: string; input?: unknown },
						ctx: ExtensionContext,
					) => Promise<{ block?: boolean; reason?: string } | undefined>,
				) => {
					if (event === "tool_call") {
						toolCallHandler = handler;
					}
				},
			),
		} as unknown as ExtensionAPI;

		registerExtension(mockPi);

		return {
			dispatchToolCall: (
				event: { toolName: string; input?: unknown },
				ctx = mockContext,
			) => {
				if (!toolCallHandler) throw new Error("Handler not registered");
				return toolCallHandler(event, ctx);
			},
		};
	}

	it("write ツールでナンバリング違反を検知してブロックする", async () => {
		const { dispatchToolCall } = setupExtension();
		const result = await dispatchToolCall({
			toolName: "write",
			input: {
				path: "README.md",
				content: "# 1. はじめに\n\n本文",
			},
		});

		expect(result).toBeDefined();
		expect(result?.block).toBe(true);
		expect(result?.reason).toContain(
			"ドキュメントおよびコードコメント内での番号振りは禁止されています",
		);
		expect(result?.reason).toContain('行 1: "# 1. はじめに"');
	});

	it("edit ツールで edits 内の newText の違反を検知してブロックする", async () => {
		const { dispatchToolCall } = setupExtension();
		const result = await dispatchToolCall({
			toolName: "edit",
			input: {
				path: "src/main.ts",
				edits: [
					{
						newText: "// 1. 初期化処理\ninit();",
					},
				],
			},
		});

		expect(result).toBeDefined();
		expect(result?.block).toBe(true);
	});

	it("違反のない正常な write 呼び出しは許可する", async () => {
		const { dispatchToolCall } = setupExtension();
		const result = await dispatchToolCall({
			toolName: "write",
			input: {
				path: "README.md",
				content: "# はじめに\n\n- 概要\n- 手順",
			},
		});

		expect(result).toBeUndefined();
	});

	it("対象外ツール（read / bash など）は処理をスキップする", async () => {
		const { dispatchToolCall } = setupExtension();
		const result = await dispatchToolCall({
			toolName: "read",
			input: {
				path: "README.md",
			},
		});

		expect(result).toBeUndefined();
	});

	it("対象外ファイル拡張子（バイナリなど）は処理をスキップする", async () => {
		const { dispatchToolCall } = setupExtension();
		const result = await dispatchToolCall({
			toolName: "write",
			input: {
				path: "image.png",
				content: "1. something",
			},
		});

		expect(result).toBeUndefined();
	});

	it("ナンバリングではない、単位付き数値は許可する", async () => {
		const { dispatchToolCall } = setupExtension();

		const blockedResult = await dispatchToolCall({
			toolName: "write",
			input: {
				path: "README.md",
				content: "# 1.29 機能説明\n\n本文",
			},
		});
		expect(blockedResult?.block).toBe(true);

		const allowedResult = await dispatchToolCall({
			toolName: "write",
			input: {
				path: "README.md",
				content: "- 7倍のパフォーマンス向上\n- 1.29 GB のデータを処理する",
			},
		});
		expect(allowedResult).toBeUndefined();
	});
});
