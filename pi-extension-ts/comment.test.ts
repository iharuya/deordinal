import { describe, expect, it } from "vitest";
import { extractCommentContent } from "./comment.ts";

describe("comment", () => {
	it("スラッシュ形式コメント（//）の本文を抽出する", () => {
		expect(extractCommentContent("// テストコメント")).toBe("テストコメント");
		expect(extractCommentContent("  // インデント付きコメント")).toBe(
			"インデント付きコメント",
		);
	});

	it("ハッシュ形式コメント（#）の本文を抽出する", () => {
		expect(extractCommentContent("# Pythonコメント")).toBe("Pythonコメント");
	});

	it("ダッシュ形式コメント（--）の本文を抽出する", () => {
		expect(extractCommentContent("-- SQLコメント")).toBe("SQLコメント");
	});

	it("ブロックコメント・JSDocの本文を抽出する", () => {
		expect(extractCommentContent("/* インラインコメント */")).toBe(
			"インラインコメント",
		);
		expect(extractCommentContent("/* 複数行コメント開始")).toBe(
			"複数行コメント開始",
		);
		expect(extractCommentContent(" * 複数行コメント継続")).toBe(
			"複数行コメント継続",
		);
	});

	it("Shebang行を除外する", () => {
		expect(extractCommentContent("#!/usr/bin/env node")).toBeNull();
		expect(extractCommentContent("#!/bin/bash")).toBeNull();
	});

	it("C/C++ プリプロセッサディレクティブを除外する", () => {
		expect(extractCommentContent("#include <stdio.h>")).toBeNull();
		expect(extractCommentContent("#define BUFFER_SIZE 1024")).toBeNull();
		expect(extractCommentContent("#ifdef DEBUG")).toBeNull();
	});

	it("静的解析・型チェッカーディレクティブを除外する", () => {
		expect(extractCommentContent("// @ts-ignore")).toBeNull();
		expect(extractCommentContent("// @ts-expect-error")).toBeNull();
		expect(extractCommentContent("# type: ignore")).toBeNull();
		expect(extractCommentContent("/* eslint-disable */")).toBeNull();
		expect(extractCommentContent("// eslint-disable-next-line")).toBeNull();
	});

	it("通常のコード行は null を返す", () => {
		expect(extractCommentContent("const x = 1;")).toBeNull();
		expect(extractCommentContent("def hello():")).toBeNull();
		expect(extractCommentContent("SELECT * FROM users;")).toBeNull();
	});
});
