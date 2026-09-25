# deordinal

文章・コードコメント中のOrdering（順序付け）をベストエフォートで検出・剥がすCLI

```sh
cargo install --path .
cd project_path
deordinal init                  # 設定ファイルを作成
deordinal check                 # カレントディレクトリを再帰的に検査
deordinal check README.md src  # 明示したファイル・ディレクトリを検査
deordinal check -v              # 診断がないファイルも検査結果を表示
```


サポートするファイル： `.md`、`.js` / `.jsx` / `.mjs` / `.cjs`、`.ts` / `.tsx` / `.mts` / `.cts`、`.py` / `.pyi`

## デフォルトルール

- `ordered-list`: Markdown の番号付きリストをリストごとに検出。入れ子は別リスト。
- `prefix`: `1. 概要`、`1: 概要`、`A) 概要`、`(1) 概要`、`① 概要` などの文章先頭ラベル。
- `keyword-prefix`: `Step 1`、`Phase A`、`ステップ 1` などの文章先頭ラベル。ラベルだけの見出しも対象。

数値・単位・年・バージョンに見える表現を一部除外しますが、意味による必要性の判定はしません。文中の参照表現は検査しません。

## 設定

対象ファイルの指定は`glob`で、`!`で除外できます。

詳細は[JSON Schema](./configuration_schema.json)。

## Markdown の ignore

必要なOrderingには、単独行の HTML コメントによる理由付き範囲指定またはファイル指定を使います。

範囲指定：
````md
<!-- deordinal-ignore-start: この手順は順番に実行する必要がある -->
1. サービスを停止する
2. バックアップを取得する
<!-- deordinal-ignore-end -->
````

ファイル全体を除外：
````md
<!-- deordinal-ignore-file: 順序付きの操作手順書である -->
````

コード中のOrderingは不要なので内コメントに ignore 記法はありません。
