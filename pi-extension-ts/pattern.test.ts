import { describe, expect, it } from "vitest";
import { checkNumberedPattern } from "./pattern.ts";

describe("pattern", () => {
	describe("検知対象のナンバリングパターン", () => {
		it("数字 + 区切り文字を検知する", () => {
			expect(checkNumberedPattern("1. はじめに")).not.toBeNull();
			expect(checkNumberedPattern("1) 概要")).not.toBeNull();
			expect(checkNumberedPattern("1: 設定")).not.toBeNull();
			expect(checkNumberedPattern("1.1 詳細")).not.toBeNull();
			expect(checkNumberedPattern("1、準備")).not.toBeNull();
			expect(checkNumberedPattern("1．導入")).not.toBeNull();
		});

		it("アルファベット + 区切り文字を検知する", () => {
			expect(checkNumberedPattern("a. 手順")).not.toBeNull();
			expect(checkNumberedPattern("A) 手順")).not.toBeNull();
			expect(checkNumberedPattern("B: 手順")).not.toBeNull();
			expect(checkNumberedPattern("a: 手順")).not.toBeNull();
		});

		it("括弧・角括弧付き番号を検知する", () => {
			expect(checkNumberedPattern("(1) 項目")).not.toBeNull();
			expect(checkNumberedPattern("(a) 項目")).not.toBeNull();
			expect(checkNumberedPattern("(一) 項目")).not.toBeNull();
			expect(checkNumberedPattern("[1] 項目")).not.toBeNull();
			expect(checkNumberedPattern("[a] 項目")).not.toBeNull();
		});

		it("丸数字・漢数字を検知する", () => {
			expect(checkNumberedPattern("① 第一項目")).not.toBeNull();
			expect(checkNumberedPattern("② 第二項目")).not.toBeNull();
			expect(checkNumberedPattern("一、 はじめに")).not.toBeNull();
			expect(checkNumberedPattern("二. つぎに")).not.toBeNull();
		});

		it("キーワードプレフィックスを検知する", () => {
			expect(checkNumberedPattern("step 1: 初期化")).not.toBeNull();
			expect(checkNumberedPattern("Step 2. 実行")).not.toBeNull();
			expect(checkNumberedPattern("Phase A: 計画")).not.toBeNull();
			expect(checkNumberedPattern("フェーズ1: 開発")).not.toBeNull();
			expect(checkNumberedPattern("タスク 1. 調査")).not.toBeNull();
			expect(checkNumberedPattern("ステージ ① 準備")).not.toBeNull();
		});

		it("階層番号（ドット区切り）パターンを検知する", () => {
			expect(checkNumberedPattern("1.2 概要")).not.toBeNull();
			expect(checkNumberedPattern("1.29 機能説明")).not.toBeNull();
			expect(checkNumberedPattern("1.29. 詳細")).not.toBeNull();
			expect(checkNumberedPattern("1.29: 詳細")).not.toBeNull();
			expect(checkNumberedPattern("1.29： 詳細")).not.toBeNull();
			expect(checkNumberedPattern("1.29) 詳細")).not.toBeNull();
			expect(checkNumberedPattern("1.29 - 詳細")).not.toBeNull();
			expect(checkNumberedPattern("1.2.3 設計方針")).not.toBeNull();
			expect(checkNumberedPattern("1.2.3. 詳細")).not.toBeNull();
			expect(checkNumberedPattern("1.29 Overview")).not.toBeNull();
		});

		it("3桁以下の数字 + 空白 + 文字のパターンを検知する", () => {
			expect(checkNumberedPattern("1 はじめに")).not.toBeNull();
			expect(checkNumberedPattern("12 要件定義")).not.toBeNull();
		});
	});

	describe("誤検知防止（許可対象）", () => {
		it("ラテン語略語（e.g., i.e.）を許可する", () => {
			expect(checkNumberedPattern("e.g. 例えばこれ")).toBeNull();
			expect(checkNumberedPattern("i.e., すなわち")).toBeNull();
		});

		it("URLを許可する", () => {
			expect(checkNumberedPattern("https://example.com/1.html")).toBeNull();
			expect(checkNumberedPattern("http://example.com/step1")).toBeNull();
		});

		it("数値・単位表現（GB, ms, px, %, 倍 等）を許可する", () => {
			expect(checkNumberedPattern("7倍のパフォーマンス")).toBeNull();
			expect(checkNumberedPattern("7 倍のパフォーマンス")).toBeNull();
			expect(checkNumberedPattern("1.29 GB のファイル")).toBeNull();
			expect(checkNumberedPattern("1.29 GB")).toBeNull();
			expect(checkNumberedPattern("1.29GB")).toBeNull();
			expect(checkNumberedPattern("1.29 GB/s の転送速度")).toBeNull();
			expect(checkNumberedPattern("100 MB のデータ")).toBeNull();
			expect(checkNumberedPattern("3.5 mm ジャック")).toBeNull();
			expect(checkNumberedPattern("2.4 GHz 帯")).toBeNull();
			expect(checkNumberedPattern("99.9 % の稼働率")).toBeNull();
			expect(checkNumberedPattern("0.5 ms の遅延")).toBeNull();
			expect(checkNumberedPattern("0.5 ms latency")).toBeNull();
			expect(checkNumberedPattern("12.5 px のパディング")).toBeNull();
			expect(checkNumberedPattern("100 px padding")).toBeNull();
			expect(checkNumberedPattern("36.5 ℃ の温度")).toBeNull();
			expect(checkNumberedPattern("1.29 倍の高速化")).toBeNull();
			expect(checkNumberedPattern("1.29倍速")).toBeNull();
			expect(checkNumberedPattern("100 円のコスト")).toBeNull();
			expect(checkNumberedPattern("50 人のユーザー")).toBeNull();
		});

		it("暗号資産の数量表現を許可する", () => {
			expect(checkNumberedPattern("23 USDC, 1 BTC, 2 ETH")).toBeNull();
			expect(checkNumberedPattern("1 BTC")).toBeNull();
			expect(checkNumberedPattern("2 ETH")).toBeNull();
		});

		it("数値に続く助詞・接続語・比較語（という、は、以上、以下、前後 等）を許可する", () => {
			expect(checkNumberedPattern("1.29 という数値")).toBeNull();
			expect(checkNumberedPattern("1.29 は 1.3 未満である")).toBeNull();
			expect(checkNumberedPattern("1.29 について考察する")).toBeNull();
			expect(checkNumberedPattern("1.29 に対して実行する")).toBeNull();
			expect(checkNumberedPattern("1.29 から 2.0 まで")).toBeNull();
			expect(checkNumberedPattern("1.29 などの値")).toBeNull();
			expect(checkNumberedPattern("1.0.0 のリリース")).toBeNull();
			expect(checkNumberedPattern("1.29 以下の場合はスキップ")).toBeNull();
			expect(checkNumberedPattern("1.29 以上の場合は警告")).toBeNull();
			expect(checkNumberedPattern("1.29 未満のデータ")).toBeNull();
			expect(checkNumberedPattern("1.29 前後で推移する")).toBeNull();
			expect(checkNumberedPattern("1.29 程度を想定")).toBeNull();
		});

		it("数式・演算子を許可する", () => {
			expect(checkNumberedPattern("1.29 ± 0.05")).toBeNull();
			expect(checkNumberedPattern("1.29 = x")).toBeNull();
			expect(checkNumberedPattern("1.29 < 2.0")).toBeNull();
		});

		it("4桁の年号を許可する", () => {
			expect(checkNumberedPattern("2024年のロードマップ")).toBeNull();
			expect(checkNumberedPattern("2025 年の目標")).toBeNull();
		});

		it("ナンバリングのない通常テキストを許可する", () => {
			expect(checkNumberedPattern("ユニットテストの導入")).toBeNull();
			expect(checkNumberedPattern("ユーザー認証を実装する")).toBeNull();
			expect(checkNumberedPattern("- 箇条書き項目")).toBeNull();
			expect(checkNumberedPattern("")).toBeNull();
			expect(checkNumberedPattern("   ")).toBeNull();
		});
	});
});
