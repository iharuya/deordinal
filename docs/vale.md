# Valeはこのプロジェクトには合わない

https://docs.vale.sh/

Valeは「文章lint基盤」としては強いが、`deordinal` は prose lint より semantic/structural lint に寄っているため、早晩Valeの抽象化からはみ出す。

さらに、`--unsafe-fix`、suppression、 classifier/Jev 連携といった高度な機能を見据えると、diagnostic engine 自体を自分で設計したいので、Valeのルール実行モデルに乗るメリットが薄い。

