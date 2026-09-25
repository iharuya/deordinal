# deordinal

LLMは、大した意味もなく `1.`, `2.`, `Phase A`, `Step 1` などの番号・段階ラベルを付けがちである。こうした表現は、後から項目の追加・削除・並べ替えが発生した際に不要な修正コストや変更上の制約になる。

Markdown に限らず、ソースコードのコメント、ドキュメント、設定ファイルなどを対象に（意味を持たない）順序付けを検出し、警告または除去するBiomeのようなツールを作る。

ただし、その「意味の持つかどうか」の判定は恣意性を孕むので、どのように扱うかは未定。

## 参考実装

Pi Coding Agentの拡張機能として似たようなことをしていた。その実装・テスト（Typescript）を `./pi-extension-ts`以下に参考として載せる。このテストは然るべき環境で全てPassすることを確認済。編集はしないこと。

## Ignore

手順書、チュートリアルなどで、Orderingをすることが有益である場合にこのLinterが無視できるようにする。

```md
<!-- deordinal-ignore: the recovery procedure must be executed in this exact order -->
1. Stop the primary database.
2. Promote the replica.
3. Update the application connection target.
4. Restart application workers.
5. Verify writes against the new primary.
```

次の1行ではなく次のsequesnceをsuppressできる。

明示的に
```md
<!-- deordinal-ignore-start: reason -->
...
<!-- deordinal-ignore-end -->
```
や
```md
<!-- deordinal-ignore-file: この文書の例示は検査対象から除外する -->
```
もサポートする。

ただし、プログラミング言語のコメントにおいてはいかなるOrderingも不要だと思うので、それらではIgnore記法をサポートしない。

## Development

- `cargo test`
- `cargo clippy --all-targets -- -D warnings` 
