# bfilter

標準入力のバイナリをバイトオフセットで読み書きし、標準出力へ出力します。

```sh
cargo build --release

# offset 0x10 の word を LE で 0x1234 に置換し、全体を出力
bfilter -s -w -L 0x10 0x1234 < input.bin > output.bin

# offset 16 から qword（8バイト）をそのまま抽出
bfilter -g -q 16 < input.bin > extracted.bin

# offset 0 の dword を BE として解釈し、指定マスクで AND
bfilter -a -d -B 0 0xffff0000 < input.bin > output.bin

# offset 0b100 の byte に指定ビットを OR
bfilter -o -b 0b100 0b10000000 < input.bin > output.bin
```

`bfilter` はビルドした `target/release/bfilter` に置き換えて実行できます。

- 操作は `-s`（set）、`-g`（get）、`-a`（AND）、`-o`（OR）のいずれか必須。
- サイズは `-b`（1）、`-w`（2）、`-d`（4）、`-q`（8バイト）のいずれか必須。
- `-L` は LE、`-B` は BE。省略時は LE。
- offset と value は符号なし整数で、10進数・`0x` の16進数・`0b` の2進数に対応。
- get は対象バイト列だけを出力します。バイト順は変えません。value は指定できず、入力不足はエラーです。
- set / AND / OR は value が必須で、対象以外のバイトを保持した全体を出力します。入力不足はゼロで拡張します。
- value が指定サイズに収まらない場合はエラーです。qword は `u64::MAX` まで扱えます。
- offset は `u64` の範囲で扱い、offset + サイズがこの範囲を超える場合はエラーです。
- エラーは標準エラー出力へ表示し、終了コードは非ゼロになります。
- 入力は逐次処理し、入力全体をメモリに保持しません。

検証は `cargo test` で実行できます。全操作・サイズ・バイト順、ゼロ拡張、入力不足、不正な引数をCLI経由で確認します。
