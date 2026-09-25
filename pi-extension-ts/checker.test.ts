import { describe, expect, it } from "vitest";
import { checkContent, isDocFile, isTargetFile } from "./checker.ts";

describe("checker", () => {
	describe("isTargetFile", () => {
		it("ドキュメントファイルを対象として判定する", () => {
			expect(isTargetFile("README.md")).toBe(true);
			expect(isTargetFile("docs/guide.markdown")).toBe(true);
			expect(isTargetFile("note.txt")).toBe(true);
			expect(isTargetFile("doc.adoc")).toBe(true);
		});

		it("ソースコードファイルを対象として判定する", () => {
			expect(isTargetFile("src/index.ts")).toBe(true);
			expect(isTargetFile("app.py")).toBe(true);
			expect(isTargetFile("main.go")).toBe(true);
			expect(isTargetFile("lib.rs")).toBe(true);
			expect(isTargetFile("script.sh")).toBe(true);
			expect(isTargetFile("Dockerfile")).toBe(true);
			expect(isTargetFile("Makefile")).toBe(true);
		});

		it("対象外ファイルを判定から除外する", () => {
			expect(isTargetFile("image.png")).toBe(false);
			expect(isTargetFile("bundle.wasm")).toBe(false);
			expect(isTargetFile("data.bin")).toBe(false);
		});
	});

	describe("isDocFile", () => {
		it("ドキュメント拡張子を判定する", () => {
			expect(isDocFile("README.md")).toBe(true);
			expect(isDocFile("note.txt")).toBe(true);
			expect(isDocFile("src/index.ts")).toBe(false);
		});
	});

	describe("checkContent (ドキュメントファイル)", () => {
		it("見出し内のナンバリングを検出する", () => {
			const md = `# 1. はじめに\n\n## 2) 概要\n\n本文`;
			const violations = checkContent(md, "README.md");
			expect(violations).toHaveLength(2);
			expect(violations[0].line).toBe(1);
			expect(violations[1].line).toBe(3);
		});

		it("箇条書き・タスクリスト内のナンバリングを検出する", () => {
			const md = `- 1. 第一項目\n- [ ] 2. 第二タスク\n- 1.29 機能説明`;
			const violations = checkContent(md, "README.md");
			expect(violations).toHaveLength(3);
			expect(violations[0].line).toBe(1);
			expect(violations[1].line).toBe(2);
			expect(violations[2].line).toBe(3);
		});

		it("箇条書き内の単位付き数値や通常数値を許可する", () => {
			const md = `- 7倍のパフォーマンス向上\n- 7 倍の高速化\n- 1.29 GB の大容量データをダウンロードする\n- 0.5 ms の低レイテンシを実現\n- 100 px の余白を設定`;
			const violations = checkContent(md, "README.md");
			expect(violations).toHaveLength(0);
		});

		it("フロントマター内の記述を無視する", () => {
			const md = `---\ntitle: 1. はじめに\n---\n\n# タイトル`;
			const violations = checkContent(md, "README.md");
			expect(violations).toHaveLength(0);
		});

		it("フェンスドコードブロック内の記述を無視する", () => {
			const md = "```ts\n// 1. コード内のコメント\nconst x = 1;\n```";
			const violations = checkContent(md, "README.md");
			expect(violations).toHaveLength(0);
		});

		it("HTMLコメント内の記述を無視する", () => {
			const md = `<!-- 1. メモ -->\n\n本文`;
			const violations = checkContent(md, "README.md");
			expect(violations).toHaveLength(0);
		});

		it("Markdownテーブル行を無視する", () => {
			const md = `| 1. 列 | 2. 列 |\n|---|---|\n| a | b |`;
			const violations = checkContent(md, "README.md");
			expect(violations).toHaveLength(0);
		});
	});

	describe("checkContent (ソースコードファイル)", () => {
		it("コメント内のナンバリングを検出する", () => {
			const code = `// 1. 初期化処理\nconst x = 1;\n/* 2. 終了処理 */`;
			const violations = checkContent(code, "index.ts");
			expect(violations).toHaveLength(2);
			expect(violations[0].line).toBe(1);
			expect(violations[0].type).toBe("code_comment");
			expect(violations[1].line).toBe(3);
		});

		it("コード本体の数字定義は無視する", () => {
			const code = `const step1 = 1;\nconst arr = [1, 2, 3];`;
			const violations = checkContent(code, "index.ts");
			expect(violations).toHaveLength(0);
		});
	});
});
