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

キーマップ編集のホスト側ツールで 2 種類あります（後述の「Vial 版と Rynk 版」）:
`aerogu34_*.uf2` が **Vial 版**（標準）、`aerogu34_*_rynk.uf2` が **Rynk 版**
（RMK 純正 GUI、実験的）。左右で同じ版を書いてください。

## 状態

実機で確認済み: 全キー、Vial、Rynk (gui.rmk.rs)、トラックボール（カーソル /
オートマウスレイヤ / スクロール）、BLE ホスト接続とプロファイル切替、LED。
未確認: 起動時のバッテリ残量表示とホストへの残量報告。設計と進捗は
[docs/DESIGN.md](docs/DESIGN.md)。

**ZMK 版との違いで知っておくこと**

- **ファームウェアを更新すると Vial / Rynk での編集は `keyboard.toml` の内容に
  戻ります**（RMK はビルドごとに変わるハッシュで設定領域を初期化する）。
  v0.1.2 からは **BLE のボンドと左右のペアリングはそのまま残る**ので、更新後に
  ホストとペアリングし直す必要はありません（fork の `keep_bonds`）。
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

### バッテリ残量（両側、それぞれ自分の電池）

起動後の最初の測定で 2 秒点灯: **緑** 40 % 以上 / **黄** 20〜39 % / **赤** 20 % 未満
（rgbled-widget と同じ閾値）。以降は 10 % を切ってさらに減るたびに赤の短い点滅。

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
`keyboard.toml` の内容に戻ります**（設定領域が初期化されるため。BLE の
ペアリングは残ります）。

## カスタマイズ: Vial でできること / ソースを変えること

### Vial で変えられる（ビルド不要、フラッシュに保存される）

| 項目 | 備考 |
| --- | --- |
| 8 レイヤ全部のキー配置 | レイヤ 0〜4 が初期キーマップ、**5〜7 は空き**。レイヤ数自体はビルド時に固定で、Vial / Rynk からは増やせない（→ [よくある変更のやり方 4](#4-レイヤを増やす--空きレイヤを使う)） |
| Mod-Tap / Layer-Tap の追加・変更 | Vial で作ったものは `[behavior.morse]` の既定値（300 ms、permissive hold）で動く |
| コンボの編集 | 枠 **48 個**（初期キーマップが 8 個使用） |
| タップダンス（RMK では "morse"） | 枠 **48 個** |
| マクロ | 全体で 512 バイト |
| BLE プロファイルキー `BT0`〜`BT4` / `CLR_BT`、マウスボタン / ホイール | User キーコードとして選べる |
| QMK Settings タブ: コンボのタイムアウト、タッピングターム、ワンショット、permissive hold | タッピングタームは**既定プロファイルにだけ**効く。ホームロウ Mod (HRM) と親指 (THUMB) は `keyboard.toml` の値のまま |

### ソースを変える必要があるもの

**トラックボール（感度・向き・デッドゾーン）、オートマウスレイヤ、
スクロールレイヤ、レイヤ数** は Vial にも Rynk にも設定画面がありません。
`keyboard.toml`（一部は `src/*.rs`）を編集してビルドし直す必要があります。
よく変える項目の具体的な手順は[よくある変更のやり方](#よくある変更のやり方コピペ用)に
まとめてあります。**手元に Rust 環境が無くても、GitHub でフォークして
ブラウザで編集すれば Actions がビルドしてくれます**（後述）。

| 変えたいこと | 場所 | 例 |
| --- | --- | --- |
| トラックボールの感度 (CPI) | `keyboard.toml` `[[split.central.input_device.paw3222]]` | `cpi = 800`（608〜4826、38 刻み） |
| カーソルの向き | 同上 | `invert_x = true` / `invert_y = true` |
| 静止時のノイズで勝手にマウスレイヤに入る | 同上 | `deadzone_threshold = 10`（カウント数、0 で無効）/ `deadzone_timeout_ms = 300` |
| オートマウスレイヤの対象レイヤ・戻るまでの時間・入らないレイヤ | `keyboard.toml` `[[behavior.auto_mouse_layer]]` | `target_layer = 4` / `timeout = "1000ms"` / `exclude_layers = [3]` |
| スクロールレイヤの番号、スクロール速度・向き、カーソル倍率 | `src/pointing_mode.rs` | `SCROLL_LAYER` / `MOUSE_LAYER`、`SCROLL_MODE` の `divisor_x/y`（大きいほど遅い）と `invert_x/y`、`CURSOR_MODE` の `multiplier_x/y` |
| ホームロウ Mod / 親指のタップホールド時間 | `keyboard.toml` `[behavior.morse.profiles]` | `HRM = { ..., hold_timeout = "300ms", gap_timeout = "300ms" }` / `THUMB = { ... }` |
| レイヤ数（8 より増やす） | `keyboard.toml` `[keymap] layers` + `[[keymap.layer]]` を追加 | |
| コンボ / タップダンスの枠 | `keyboard.toml` `[rmk]` | `combo_max_num = 48` / `morse_max_num = 48`（上限 255。1 コンボのキー数は `combo_max_length`、既定 4） |
| マクロの容量 | `keyboard.toml` `[rmk]` | `macro_space_size = 512`（バイト。RAM を 1 バイトあたり約 23 バイト消費する） |
| BLE プロファイル数 | `keyboard.toml` `[rmk]` | `ble_profiles_num = 5`（Vial のカスタムキーは `vial.json` の `customKeycodes` も合わせる） |
| 無操作からスリープまでの時間 | `keyboard.toml` `[rmk]` | `split_central_sleep_timeout_seconds = 30` |
| 切替後の再接続の速さ / 消費電力 | `keyboard.toml` `[ble]` | `advertising_fast_interval_ms = 30` / `advertising_fast_timeout_secs = 30` |
| LED の色・パターン | `src/status_led.rs` | `RED` `BLUE` … と `host_status()` / `link_status()` |
| キーマップの初期値 | `keyboard.toml` `[[keymap.layer]]` | 行は物理配置どおり（左 5 → 右 5、親指 2+2） |

`keyboard.toml` の各項目にはコメントで理由を書いてあります。RMK 側の全項目は
[RMK の設定リファレンス](https://rmk.rs/main/docs/configuration/appendix)を参照。

### よくある変更のやり方（コピペ用）

**どれも Vial / Rynk からは変えられません。** 下記を編集してビルドし直し、
出来た UF2 を書き込みます（ビルドは「フォークしてブラウザだけでビルドする」
の手順で、PC に開発環境は不要）。右半分だけで効く設定も、左右で設定を
揃えておく方が混乱しません（トラックボール関連は右だけで効きます）。

#### 1. オートマウスレイヤを使わない

ボールを動かしてもレイヤが切り替わらなくなります。`keyboard.toml` の
`[[behavior.auto_mouse_layer]]` ブロックを**丸ごと削除**（または各行の頭に
`#` を付けてコメントアウト）してください。それだけで済みます
（`[event.pointing]` などの購読枠はそのままでも動きます。余らせても
数十バイトの無駄だけです）。

レイヤ 4（Mouse）はキーマップに残るので、`MO(4)` などを置いて手動で
使うこともできます。

#### 2. オートマウスレイヤの設定を変える

```toml
[[behavior.auto_mouse_layer]]
device_id = 0
target_layer = 4        # 切り替わり先のレイヤ番号（0〜7）
timeout = "1000ms"      # ボールを止めてから戻るまで。"600ms" など
threshold = 1           # 反応する最小の移動量。上げると鈍くなる
exclude_layers = [3]    # このレイヤが有効な間は切り替わらない（スクロールレイヤ）
```

`target_layer` を変えたら、**そのレイヤのキーマップ**（`[[keymap.layer]]` の
5 番目 = レイヤ 4 が現在の Mouse）も意図したものになっているか確認してください。
`src/pointing_mode.rs` の `MOUSE_LAYER` も同じ番号に合わせます。

#### 3. スクロールレイヤの番号・速度・向きを変える

`src/pointing_mode.rs` の冒頭にある定数です:

```rust
const SCROLL_LAYER: u8 = 3;   // このレイヤの間だけボールがホイールになる
const MOUSE_LAYER: u8 = 4;    // オートマウスレイヤ（2 と同じ番号にする）

const SCROLL_MODE: PointingMode = PointingMode::Scroll(ScrollConfig {
    multiplier_x: 1,
    divisor_x: 12,     // 横スクロール。大きいほど遅い
    multiplier_y: 1,
    divisor_y: 12,     // 縦スクロール。6 にすると倍速
    invert_x: true,    // 横の向き
    invert_y: true,    // 縦の向き（「下に転がすと上にスクロール」なら反転）
});

const CURSOR_MODE: PointingMode = PointingMode::Cursor(CursorConfig {
    multiplier_x: 1,   // カーソル速度。2 で倍速（細かい調整は CPI 側で）
    multiplier_y: 1,
    invert_x: false,
    invert_y: false,
});
```

`SCROLL_LAYER` を変えたら、`keyboard.toml` の `exclude_layers` も同じ番号に
してください（そうしないとスクロール中にオートマウスレイヤが被さります）。

#### 4. レイヤを増やす / 空きレイヤを使う

**レイヤ 5・6・7 は空**（全部透過）で用意してあるので、**増やさずに Vial で
中身を作れます**。呼び出すキー（`MO(5)` や `LT(5, Space)`）も Vial で置けます。

8 より増やしたいときだけ `keyboard.toml` を 2 か所:

```toml
[keymap]
layers = 10          # <- 増やした数
```

```toml
# 末尾の [[keymap.layer]] をコピーして、増やした数だけ並べる
[[keymap.layer]]
name = "user8"
keys = """
_ _ _ _ _  _ _ _ _ _
_ _ _ _ _  _ _ _ _ _
_ _ _ _ _  _ _ _ _ _
_ _  _ _
"""
```

`layers` の数と `[[keymap.layer]]` の個数は一致させてください（合わないと
ビルドが止まります）。レイヤ 1 つあたり RAM 280 バイト（実測）なので、
増やすこと自体のコストはほぼありません。

#### 5. トラックボールの感度・ノイズ対策

```toml
[[split.central.input_device.paw3222]]
cpi = 800                   # 608〜4826、38 刻み。上げると速い
invert_x = true             # カーソルの向き（スクロールは 3 の invert_* 側）
invert_y = true
deadzone_threshold = 10     # 静止時のノイズを無視するカウント数。0 で無効
deadzone_timeout_ms = 300   # 無動作でデッドゾーンが再び有効になるまで
```

「机を叩くとマウスレイヤに入る」なら `deadzone_threshold` を上げ、
「ゆっくり動かし始めが無視される」なら下げてください。

### Vial 版と Rynk 版

RMK には Vial のほかに純正のホストプロトコル **Rynk** があり、
GUI は <https://gui.rmk.rs/>（Chrome / Edge、WebUSB）。二つは排他なので
ファームウェアが 2 種類あります。

| | Vial 版 `aerogu34_*.uf2` | Rynk 版 `aerogu34_*_rynk.uf2` |
| --- | --- | --- |
| ツール | [Vial](https://get.vial.today/)（デスクトップ / Web、USB・BLE） | [gui.rmk.rs](https://gui.rmk.rs/)（USB、WebUSB） |
| キーマップ / コンボ / タップダンス / マクロ | ○ | ○ |
| タップホールドの詳細（HRM / THUMB プロファイル別の hold timeout、permissive hold、flow tap、quick tap …） | △ 既定プロファイルの timeout のみ | **○** |
| BT パネル、既定レイヤ、レイアウトバリアント | △ キーとして | ○ 専用画面 |
| 状態表示（レイヤ、バッテリ、接続、左右） | × | ○ |
| トラックボール設定・オートマウスレイヤ・スクロールレイヤ | × | × どちらも不可（`keyboard.toml` / `src/pointing_mode.rs` で設定） |
| 成熟度 | RMK の既定。安定 | **実験的**。プロトコルが RMK のリリースごとに変わりうる。gui.rmk.rs が新しい RMK を前提にしていて噛み合わないことがある |

迷ったら Vial 版。Rynk 版は「RMK らしい UI を試したい」人向けで、動かなければ
Vial 版に戻してください。**キーマップは両版で同じ形式で保存されている**ので、
焼き替えても Vial / Rynk で編集した内容は引き継がれます（ファームウェアが
変わるので初回起動時に再同期が走ります。BLE のペアリングは残ります）。

### フォークしてブラウザだけでビルドする

1. このリポジトリを GitHub で **Fork**
2. 自分のフォークの **Actions** タブを開き、ワークフローを有効化（フォーク直後は無効）
3. ブラウザで `keyboard.toml`（や `src/*.rs`）を編集して main に commit
4. 数分で Actions が終わるので、その run の **Artifacts → `firmware`** から UF2 を
   ダウンロードして書き込む

タグ `vX.Y.Z` を打てば Release ページにも UF2 が付きます（`.github/workflows/build.yml`）。

**注意**: `keyboard.toml` のキーマップ・動作設定は、新しいファームウェアを
書いたときに一度だけ反映され、そのとき Vial の編集は消えます（BLE の
ペアリングは残ります）。

## 入手

- [Releases](https://github.com/t-ogura/rmk-keyboard-aerogu34/releases) —
  タグごとの UF2（推奨）
- [firmware/](firmware/) — main の最新ビルド（Vial 版・Rynk 版の 4 ファイル）
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

./package.sh              # Vial 版 -> firmware/aerogu34_{right,left}.uf2
./package.sh --host rynk  # Rynk 版 -> firmware/aerogu34_{right,left}_rynk.uf2
./package.sh --host both  # 両方（CI と同じ）
./package.sh --dev        # keyboard.toml のキーマップ変更を rmk の再ビルド無しで反映（下記）
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
