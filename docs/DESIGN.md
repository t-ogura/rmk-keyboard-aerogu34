# Aerogu34 / RMK 移植 設計メモ

**対象**: Aerogu34（Xiao BLE ×2、右に PAW3222）を ZMK から RMK へ移植する。
**作成日**: 2026-09-14
**RMK**: `t-ogura/rmk` `paw3222` ブランチ（上流 `rmk-rs/rmk` main @ `4e432b4`、2026-09-11 + PAW3222 ドライバと修正群）
**移植元**: `zmk-keyboard-aerogu34` main @ `e3772f2`
**先行事例**: `../../cornix/rmk-cornix-tb/`（Cornix TB の RMK 化。`docs/DESIGN.md` に
RMK の落とし穴が 2,000 行分ある。本書はそれを繰り返さず、Aerogu34 で**違う点**と
**決めたこと**だけを書く）

---

## 1. 位置づけ

ZMK 版は現役の配布物（テスター向け）で、そのまま残す。本プロジェクトは並行する
別実装。Cornix TB と違い **Aerogu34 にはユーザーがいる**ので、他の人が
ビルド・フラッシュできる整備（fork の公開、CI、README）を最初から含める。

## 2. Cornix TB との違い（＝ 楽になる点）

| | Cornix TB | Aerogu34 |
| --- | --- | --- |
| 構成 | Xiao (キーレス central) + Cornix 左右 (peripheral ×2) | Xiao 右 (central、キー + トラックボール) + Xiao 左 (peripheral ×1) |
| フラッシュ配置 | 0x27000 (Xiao) と 0x1000 (Cornix) が混在 → 取り違え事故 | 全部 0x27000。`memory.x` 1 つ |
| DC-DC | Cornix は禁止 (インダクタ無し) | Xiao は REG1 を使える (ZMK も使っていた) |
| LED | Cornix は WS2812 (PWM 自作) | 両側とも Xiao 内蔵 RGB (GPIO 3 本) |
| peripheral 数 | 2 (0.9.0 の購読枠バグを踏んだ) | 1 |
| バッテリ | 3 種類のピン | 両側とも P0.31 + P0.14 enable |
| キーマップ | 50 キーのうち 34 を使う | 34 キーそのもの。Cornix 側で移植済み (`efbb77d`) |

そのまま持ち込んだ Cornix 資産: `memory.x`、`hfxo.rs` (HFXO 起動 + GPREGRET2
マーカー)、`build.rs` (デバイス名 22 バイト検査、Vial 定義生成、`RMK_FEATURES`
転送)、`status_led.rs` (中身は central / peripheral 両用に書き直し)、
`pointing_mode.rs` (レイヤ別スクロール切替)、`package.sh` (UF2 化 + 配置検査)、
`keyboard.toml` のキーマップ・エイリアス・コンボ・morse プロファイル。

## 3. ハードウェア

Xiao BLE のピン対応: D0 P0.02, D1 P0.03, D2 P0.28, D3 P0.29, D4 P0.04, D5 P0.05,
D6 P1.11, D7 P1.12, D8 P1.13, D9 P1.14, D10 P1.15。P0.09 / P0.10 は NFC パッド
(`nfc-pins-as-gpio`)。

| 機能 | ピン | 出典 |
| --- | --- | --- |
| rows (入力、pull-down) | P0.03, P0.28, P0.29, P1.11 | `aerogu34.dtsi` row-gpios (D1 D2 D3 D6) |
| cols (出力) | P1.15, P1.14, P1.13, P1.12, P0.10 | col-gpios (D10 D9 D8 D7 P0.10) |
| ダイオード | col2row (RMK 既定) | `diode-direction = "col2row"` |
| トラックボール SCK / SDIO / CS / MOTION | P0.05 / P0.04 / P0.09 / P0.02 | `aerogu34.dtsi` spi0 pinctrl + `aerogu34_right.overlay` |
| バッテリ ADC / divider enable | P0.31 (510k / 1510k) / P0.14 (active low) | Xiao BLE 回路図、Cornix central と同じ |
| RGB LED (共通アノード) | R P0.26, G P0.30, B P0.06 | Xiao BLE |
| 左右の違い | 無し（同一 PCB） | |

## 4. マトリクスとキーマップ

**統合マトリクス 4×10**、右が `col_offset = 5`。ZMK の `default_transform`
（右 `col-offset = <5>`）と DYA Studio 用 `config/aerogu34.json` の `(row, col)`
と同じ番号で、キーの名前が両ファームで一致する。

同一 PCB を左右で使うので各半分の col 0 は内側。`[layout] map` は物理順:

```
(0,4) (0,3) (0,2) (0,1) (0,0) [3] (0,5) (0,6) (0,7) (0,8) (0,9)
...
[3.5] (3,1) (3,0) [2] (3,5) (3,6)
```

`[[keymap.layer]].keys` はこの順なので、ZMK の keymap と行ごとに 1:1 で読める。

### ZMK → RMK 対応（Cornix 側で確定済み、`keyboard.toml` にコメントあり）

| ZMK | RMK |
| --- | --- |
| `&mt MOD KEY` (`balanced`, 300 ms) | `MT(KEY, MOD, HRM)` — `permissive_hold`, 300 ms |
| `&lt N KEY` (`quick-tap-ms 300`) | `LT(N, KEY, THUMB)` — `hold_on_other_press`, quick_tap 300 ms |
| コンボ (物理位置) | `[behavior.combo]` (キーコードで指定、位置→キーコードに翻訳) |
| `&bt BT_SEL n` / `BT_CLR` | `User0..4` / `User7` (`[rmk] ble_profiles_num = 5`) |
| `&bootloader` | `Bootloader` |
| `&studio_unlock` | 無し → `_` |
| `JP_*` define | `[aliases]` |
| `&zip_xy_transform (X_INVERT\|Y_INVERT)` | センサ側 `invert_x/y = true` |
| `res-cpi 800` | `cpi = 800` |
| `&zip_temp_layer 4 1000` | `[[behavior.auto_mouse_layer]] target_layer = 4, timeout = 1000ms, exclude_layers = [3]` |
| `scroll_runtime_input_processor` (layer 3) | `src/pointing_mode.rs`: レイヤ 3 で `Scroll` 1/12、他は `Cursor` 1:1 |
| `deadzone_processor` (10, 300 ms) | `deadzone_threshold = 10`, `deadzone_timeout_ms = 300`（fork で `PointingDevice` に追加、§8） |
| `mouse_runtime_input_processor` (DYA Studio) | 無し（Vial / Rynk はセンサ設定を持たない） |
| axis-snap Y (200) | 無し |

タップホールドの 300 ms は ZMK 版の値。Cornix で同じキーマップを RMK に載せた
ときは 200 ms に落ち着いた（純粋なホールドの遅さ）。ユーザーの慣れは ZMK 版の
300 なので、まず 300 で出す。

## 5. フラッシュ / ストレージ

- app: 0x27000〜（s140 の後ろ。ZMK と同じ）。`memory.x` は LENGTH 788K で
  0xEC000 まで
- storage: **0xE4000〜 8 セクタ**（RMK 既定の 0xA0000 はイメージが大きくなると
  踏む。Cornix で実際に踏んだ）。`package.sh` が各 UF2 の終端 < 0xE4000 を検査
- ZMK の settings 領域 (0xEC000〜、bootloader の手前) とは重ならない。RMK ⇄ ZMK
  の行き来にリセット手順は不要（ZMK 側の BLE ボンドは ZMK の領域に残る）
- `clear_layout = true`: キーマップ確定まで。Vial 編集は再起動で消える

## 6. LED

`status_led.rs` を central / peripheral 両用に。表示は Cornix の Xiao central と
同じ（リセット理由 3 s → 接続状態 3 s → 消灯、変化時 3 s）。

右 (central) は `ConnectionStatusChangeEvent` も購読し、BLE プロファイル /
状態が変わるとプロファイル番号 + 1 回の点滅（青 = Connected、黄 = Advertising、
Inactive は表示しない）。`[event.connection_status_change] subs = 2`。
ZMK の rgbled-widget にあった**起動時のバッテリ残量表示**は未実装
（`BatteryStatusEvent` の購読枠を足せば書ける）。

## 7. 進捗

### Phase A — 骨組みとビルド ✅ (2026-09-14)

- 単一 crate、2 バイナリ (`right` = `#[rmk_central]`, `left` = `#[rmk_peripheral(id = 0)]`)
- `cargo build --release` 両方通過。`cargo expand` で DC-DC reg1 = true、
  マトリクスピン、`Matrix<.., 4, 5, true, 0, 5>`、トラックボールピンを確認
- `package.sh` で `firmware/aerogu34_{right,left}.uf2`。右 0x27000..0x99400、
  左 0x27000..0x6EF00
- fork を `t-ogura/rmk` に公開し (paw3222 + pr/* 8 本)、`Cargo.toml` は
  `git` + `rev` 固定。手元は `~/workspace/aerogu34/.cargo/config.toml` の
  `[patch]` で `cornix/rmk-cornix-tb/vendor/rmk` に向けている（リポジトリ外）。
  patch 無しで GitHub から解決してビルドできることも確認済み
- GitHub Actions (`.github/workflows/build.yml`): libclang を apt、
  cargo-binstall で flip-link / cargo-binutils / cargo-hex-to-uf2、`package.sh`

### Phase B — 実機 (進行中)

確認順:
1. ✅ 右だけ USB → 右 15 キーが打てる、LED が赤 (左未接続)
2. ✅ 左を起動 → 数秒で両方の LED が緑、左 19 キーが打てる
3. トラックボール: カーソルの向き (`invert_x/y`)、速度、オートマウスレイヤ、
   スクロールレイヤの向き (`SCROLL_MODE`)
4. ✅ Vial 接続、Matrix Tester
5. BLE ホスト接続、プロファイル切替 (User0..4)、バッテリ表示
6. 起動時間（Cornix では 19 秒。保存済みアドレスへの 15 秒接続タイムアウトが濃厚）

「完全に無反応」のときの切り分け順は Cornix DESIGN §11.0 / §11.8:
デバイス名 ≤ 22 バイト → HFXO → 最小構成から足す。

### Phase C — 整備

- Cornix と共通のコード (`hfxo` / `status_led` / `build.rs` 補助 / `memory.x`)
  を支援 crate に抽出し、Cornix 側も差し替え
- LED: バッテリ残量
- `clear_layout = false` に戻す

## 8. fork に足したもの (Aerogu34 起点)

| コミット | 内容 | 上流へ |
| --- | --- | --- |
| `b93572ca` feat(pointing): optional deadzone on PointingDevice | `PointingDevice` にデッドゾーン（バーストの合計が `threshold` に達するまで報告しない、`timeout` 無動作でリセット）。paw3222 / pmw3610 / pmw33xx の `deadzone_threshold` / `deadzone_timeout_ms`。ZMK の `zmk-input-processor-deadzone` と同じ意味論 | 出す価値あり。`pr/pointing-deadzone` に切り出し予定（Cornix `docs/UPSTREAM_PRS.md` の流儀） |

なぜドライバ (paw3222.rs) ではなく `PointingDevice` か: 静止時の迷いカウントは
センサ共通の性質で、オートマウスレイヤ側の `threshold` はレポート単位
（125 Hz なら 8 ms 分）なのでノイズを弾く値にすると遅い動きの出だしも弾く。
ZMK と同じく「バーストの累積」で判定するにはレポート生成の直前が正しい場所。
