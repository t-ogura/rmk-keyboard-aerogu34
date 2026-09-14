# rmk-keyboard-aerogu34

[RMK](https://github.com/rmk-rs/rmk)（Rust 製キーボードファームウェア）版の
Aerogu34 ファームウェアです。ZMK 版
([zmk-keyboard-aerogu34](https://github.com/t-ogura/zmk-keyboard-aerogu34))
と同じハードウェア（Seeed Xiao BLE ×2、右側に PAW3222 トラックボール）で動く
**並行実装**で、ZMK 版を置き換えるものではありません。

| 半分 | 役割 | 書き込むファイル |
| --- | --- | --- |
| 右 | split central — USB / BLE でホストに接続、トラックボール、キーマップ、Vial | `firmware/aerogu34_right.uf2` |
| 左 | split peripheral — 右へ BLE で送るだけ | `firmware/aerogu34_left.uf2` |

## 状態

実機で**キー入力と Vial** を確認済み (2026-09-14)。トラックボール・BLE ホスト
接続・バッテリ表示は確認中。`docs/DESIGN.md` に設計と進捗があります。

## 書き込み (フラッシュ)

ZMK 版と同じ手順です。

1. USB ケーブルでマイコンを PC に接続
2. リセットボタンを **2 回素早く** 押すと `XIAO-SENSE` という USB ドライブとして
   認識される (DFU モード)
3. 対応する `.uf2` をドラッグ&ドロップ（右に `aerogu34_right.uf2`、左に
   `aerogu34_left.uf2`）
4. 自動再起動して反映

ZMK 版から切り替える場合、ZMK の `settings_reset.uf2` は不要です。RMK は
ZMK とは別の領域 (0xE4000〜) に設定を置くので、初回起動時に自分で初期化
します（数秒）。左右のペアリングは自動です。

ZMK 版に戻すときは ZMK の `.uf2` をそのまま書けば戻ります。RMK の設定領域は
ZMK からは見えないので、放置して構いません。

## LED（Xiao 内蔵 RGB、左右とも）

色は ZMK 版の rgbled-widget と同じ（青 = 接続、黄 = 未登録で受付中、赤 = 未接続）。
パターンで「登録済みで待っている」と「登録が無い」を区別します。

### 右: ホストとの BLE 接続

プロファイル切替 (`BT0`〜`BT4`) のたび、およびホストの接続 / 切断のたびに表示:

| LED | 状態 |
| --- | --- |
| 青 点灯 2 秒 → 消灯 | ホストと接続完了 |
| 赤 ゆっくりブリージング（最長 30 秒） | このプロファイルは登録済み。そのホストの再接続待ち |
| 黄 素早く点滅（最長 30 秒） | このプロファイルは未登録。ペアリング受付中 |
| 消灯 | USB モード / スリープ |

### 左右の接続（両側）

もう片方が繋がると緑 2 秒、切れると赤 2 秒（点灯。ブリージングの赤とは別）。

### 起動直後 3 秒: 前回のリセット理由

| 色 | 意味 |
| --- | --- |
| 消灯 | 電源投入 / リセットボタン（正常） |
| 青 | ウォッチドッグ（ハングを検出して再起動した） |
| マゼンタ（点灯 / N 回点滅） | ファームウェア自身によるソフトリセット（N = 理由コード） |
| 白 | CPU ロックアップ |

表示していない間は PWM ごと止めるので、消費は消灯時ゼロです。

**切替後の再接続には数秒かかります。** RMK はホストとの接続を 1 本しか持たず、
切替のたびに旧ホストを切って広告し直すため（ZMK は全ホストと繋ぎっぱなしで
送り先を変えるだけなので 1 秒未満）。広告を最初の 30 秒は 30 ms 間隔にして
短縮しています（`keyboard.toml` `[ble] advertising_*`）。

## キーマップ

ZMK 版 `config/aerogu34.keymap` を `keyboard.toml` に移植したものです（5 レイヤ、
ホームロウ Mod、親指レイヤタップ、コンボ 8 個、日本語配列エイリアス）。
ZMK との違い:

- `&bt BT_SEL 0..4` は RMK の User キーコード (Vial では `BT0`〜`BT4`)。
  `&bt BT_CLR` は `CLR_BT`。スクロールレイヤの Q+T コンボが `CLR_BT`、
  W+E+R コンボがブートローダ。
- `&studio_unlock`（スクロールレイヤ左ホームロウ外側）は RMK に無いので透過
- トラックボール: 反転・CPI 800・オートマウスレイヤ (レイヤ 4、1 秒)・
  デッドゾーン (10 カウント / 300 ms) は ZMK と同じ。スクロールレイヤ (3) で
  ホイール（1/12 倍）。DYA Studio によるランタイム調整に相当するものは無い
- タップホールドは ZMK の `balanced` 300 ms 相当。速すぎ / 遅すぎは
  `keyboard.toml` の `[behavior.morse.profiles]` で調整

### Vial

右半分を USB で繋いで [Vial](https://get.vial.today/) を開けば、そのまま
編集できます（`insecure = true` なのでアンロック操作は不要）。

**注意**: 現状 `[storage] clear_layout = true` のため、Vial での編集は
**再起動すると `keyboard.toml` の内容に戻ります**。キーマップを
`keyboard.toml` で確定させたら `false` にしてください。

## ソースからビルドする

push のたびに [GitHub Actions](.github/workflows/build.yml) が両方の UF2 を
ビルドして Artifacts `firmware` に置きます。`firmware/` の UF2 も同じものです。

RMK 本体は PAW3222 ドライバとデッドゾーンを含む
[t-ogura/rmk](https://github.com/t-ogura/rmk) の `paw3222` ブランチに
コミット固定で依存します（上流への PR は準備中。`Cargo.toml` のコメント参照）。

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add thumbv7em-none-eabihf
rustup component add llvm-tools
cargo install cargo-binstall
cargo binstall flip-link cargo-binutils cargo-hex-to-uf2
# nrf-sdc / nrf-mpsl のビルドに libclang が要る (Ubuntu: apt install libclang-dev)

./package.sh              # -> firmware/aerogu34_{right,left}.uf2
./package.sh --host rynk  # Vial の代わりに RMK 純正の Rynk (https://gui.rmk.rs/)
./package.sh --log        # 右半分の RMK ログを USB シリアルに出す診断ビルド
```

`package.sh` は UF2 の配置 (0x27000〜、設定領域の手前で終わる) を検査し、
外れていれば出力を拒否します。

### 触る前に知っておくこと

- **`[keyboard] name` / `product_name` は 22 バイト以下**。超えると BLE
  スタックが起動時に panic し、USB も何も出ない「完全な沈黙」になります
  （`build.rs` がビルドで止めます）。
- `keyboard.toml` のキーマップだけ変えた場合、`clear_layout = true` でないと
  **フラッシュしても前のキーマップのまま**になります（RMK はビルドハッシュが
  変わらないと読み直さない）。
- 両側とも Xiao BLE なので `memory.x` は 1 つ、フラッシュ先を間違える事故は
  ありません。

## ライセンス

[MIT License](LICENSE) — Copyright (c) 2026 Tadashi Ogura

| 依存 | License |
| --- | --- |
| [RMK](https://github.com/rmk-rs/rmk) | MIT OR Apache-2.0 |
| [embassy](https://github.com/embassy-rs/embassy) | MIT OR Apache-2.0 |
| [nrf-sdc / nrf-mpsl](https://github.com/alexmoon/nrf-sdc) | MIT OR Apache-2.0（Nordic の SoftDevice Controller バイナリを含む: [Nordic 5-Clause](https://github.com/alexmoon/nrf-sdc/blob/main/nrf-sdc-sys/third_party/nordic/nrfxlib/LICENSE)） |
