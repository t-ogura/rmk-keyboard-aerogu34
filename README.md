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

実機で確認済み: 全キー、Vial、トラックボール（カーソル / オートマウスレイヤ /
スクロール）、BLE ホスト接続とプロファイル切替、LED。未確認: バッテリ残量の
表示。設計と進捗は [docs/DESIGN.md](docs/DESIGN.md)。

**ZMK 版との違いで知っておくこと**

- **ファームウェアを更新するとホストとの再ペアリングが必要です。** RMK は
  ビルドごとに変わるハッシュで設定領域の互換性を判定し、違えば全消去します
  （左右のペアリングは自動で復旧、ホスト側のボンドは消える）。RMK の仕様です。
- ZMK 版とは BLE アドレスが違うので、ホストには「Aerogu34」が 2 つ並びます。
  RMK 版に落ち着いたら ZMK 版のほうをホストから削除してください。
- プロファイル切替後の再接続に数秒かかります（後述）。
- ZMK Studio / DYA Studio は使えません。キーマップは Vial で編集します。

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
編集できます（`insecure = true` なのでアンロック操作は不要）。編集は
フラッシュに保存され再起動後も残りますが、**ファームウェアを更新すると
`keyboard.toml` の内容に戻ります**（設定領域が初期化されるため）。

## カスタマイズ: Vial でできること / ソースを変えること

### Vial で変えられる（ビルド不要、フラッシュに保存される）

| 項目 | 備考 |
| --- | --- |
| 5 レイヤ全部のキー配置 | レイヤ数 (5) は固定 |
| Mod-Tap / Layer-Tap の追加・変更 | Vial で作ったものは `[behavior.morse]` の既定値（300 ms、permissive hold）で動く |
| コンボの編集 | 枠は 8 個で、初期キーマップの 8 個で埋まっている。増やすには `combo_max_num`（下記） |
| タップダンス（RMK では "morse"） | 枠 8 個 |
| マクロ | 全体で 256 バイト |
| BLE プロファイルキー `BT0`〜`BT4` / `CLR_BT`、マウスボタン / ホイール | User キーコードとして選べる |
| QMK Settings タブ: コンボのタイムアウト、タッピングターム、ワンショット、permissive hold | タッピングタームは**既定プロファイルにだけ**効く。ホームロウ Mod (HRM) と親指 (THUMB) は `keyboard.toml` の値のまま |

### ソースを変える必要があるもの

トラックボールと動作パラメータは `keyboard.toml`（一部は `src/*.rs`）にあり、
変えたらビルドが要ります。**手元に Rust 環境が無くても、GitHub でフォークして
ブラウザで編集すれば Actions がビルドしてくれます**（後述）。

| 変えたいこと | 場所 | 例 |
| --- | --- | --- |
| トラックボールの感度 (CPI) | `keyboard.toml` `[[split.central.input_device.paw3222]]` | `cpi = 800`（608〜4826、38 刻み） |
| カーソルの向き | 同上 | `invert_x = true` / `invert_y = true` |
| 静止時のノイズで勝手にマウスレイヤに入る | 同上 | `deadzone_threshold = 10`（カウント数、0 で無効）/ `deadzone_timeout_ms = 300` |
| オートマウスレイヤの対象レイヤ・戻るまでの時間・入らないレイヤ | `keyboard.toml` `[[behavior.auto_mouse_layer]]` | `target_layer = 4` / `timeout = "1000ms"` / `exclude_layers = [3]` |
| スクロールレイヤの番号、スクロール速度・向き、カーソル倍率 | `src/pointing_mode.rs` | `SCROLL_LAYER` / `MOUSE_LAYER`、`SCROLL_MODE` の `divisor_x/y`（大きいほど遅い）と `invert_x/y`、`CURSOR_MODE` の `multiplier_x/y` |
| ホームロウ Mod / 親指のタップホールド時間 | `keyboard.toml` `[behavior.morse.profiles]` | `HRM = { ..., hold_timeout = "300ms", gap_timeout = "300ms" }` / `THUMB = { ... }` |
| レイヤ数 | `keyboard.toml` `[keymap] layers` + `[[keymap.layer]]` を追加 | |
| コンボ / タップダンスの枠 | `keyboard.toml` `[rmk]` | `combo_max_num = 16` / `morse_max_num = 16` |
| BLE プロファイル数 | `keyboard.toml` `[rmk]` | `ble_profiles_num = 5`（Vial のカスタムキーは `vial.json` の `customKeycodes` も合わせる） |
| 無操作からスリープまでの時間 | `keyboard.toml` `[rmk]` | `split_central_sleep_timeout_seconds = 30` |
| 切替後の再接続の速さ / 消費電力 | `keyboard.toml` `[ble]` | `advertising_fast_interval_ms = 30` / `advertising_fast_timeout_secs = 30` |
| LED の色・パターン | `src/status_led.rs` | `RED` `BLUE` … と `host_status()` / `link_status()` |
| キーマップの初期値 | `keyboard.toml` `[[keymap.layer]]` | 行は物理配置どおり（左 5 → 右 5、親指 2+2） |

`keyboard.toml` の各項目にはコメントで理由を書いてあります。RMK 側の全項目は
[RMK の設定リファレンス](https://rmk.rs/main/docs/configuration/appendix)を参照。

### フォークしてブラウザだけでビルドする

1. このリポジトリを GitHub で **Fork**
2. 自分のフォークの **Actions** タブを開き、ワークフローを有効化（フォーク直後は無効）
3. ブラウザで `keyboard.toml`（や `src/*.rs`）を編集して main に commit
4. 数分で Actions が終わるので、その run の **Artifacts → `firmware`** から UF2 を
   ダウンロードして書き込む

タグ `vX.Y.Z` を打てば Release ページにも UF2 が付きます（`.github/workflows/build.yml`）。

**注意**: `keyboard.toml` のキーマップ・動作設定は、新しいファームウェアを
書いたときに一度だけ反映され、そのとき Vial の編集と BLE ボンドは消えます。

## 入手

- [Releases](https://github.com/t-ogura/rmk-keyboard-aerogu34/releases) —
  タグごとの UF2（推奨）
- [firmware/](firmware/) — main の最新ビルド
- [GitHub Actions](../../actions) — push ごとの Artifacts `firmware`

## ソースからビルドする

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
# nrf-sdc / nrf-mpsl のビルドに libclang、BLE ペアリングの P-256 (p256-cortex-m4-sys,
# C 実装) に ARM 用 gcc が要る:  Ubuntu: apt install libclang-dev gcc-arm-none-eabi

./package.sh              # -> firmware/aerogu34_{right,left}.uf2
./package.sh --dev        # keyboard.toml のキーマップ変更を rmk の再ビルド無しで反映（下記）
./package.sh --host rynk  # Vial の代わりに RMK 純正の Rynk (https://gui.rmk.rs/)
./package.sh --log        # 右半分の RMK ログを USB シリアルに出す診断ビルド
```

`package.sh` は UF2 の配置 (0x27000〜、設定領域の手前で終わる) を検査し、
外れていれば出力を拒否します。

### 触る前に知っておくこと

- **`[keyboard] name` / `product_name` は 22 バイト以下**。超えると BLE
  スタックが起動時に panic し、USB も何も出ない「完全な沈黙」になります
  （`build.rs` がビルドで止めます）。
- `keyboard.toml` のキーマップだけ変えて手元でビルドすると、**フラッシュしても
  前のキーマップのまま**になります（RMK は rmk crate のビルド時に決まる
  ハッシュが変わらないと読み直さない）。`./package.sh --dev` は
  `clear_layout = true` にして毎起動時に上書きさせます（Vial の編集は残らない）。
  CI のようなクリーンビルドではこの問題は起きません。
- 両側とも Xiao BLE なので `memory.x` は 1 つ、フラッシュ先を間違える事故は
  ありません。

## ライセンス

[MIT License](LICENSE) — Copyright (c) 2026 Tadashi Ogura

| 依存 | License |
| --- | --- |
| [RMK](https://github.com/rmk-rs/rmk) | MIT OR Apache-2.0 |
| [embassy](https://github.com/embassy-rs/embassy) | MIT OR Apache-2.0 |
| [nrf-sdc / nrf-mpsl](https://github.com/alexmoon/nrf-sdc) | MIT OR Apache-2.0（Nordic の SoftDevice Controller バイナリを含む: [Nordic 5-Clause](https://github.com/alexmoon/nrf-sdc/blob/main/nrf-sdc-sys/third_party/nordic/nrfxlib/LICENSE)） |
