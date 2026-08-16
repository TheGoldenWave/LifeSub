# Third-Party Notices

This file is generated from the pinned model manifest and the `asr-qwen17-runtime` Cargo metadata closure. Rows between the markers are machine-readable tab-separated records; manual additions inside those sections are rejected by contract tests.

## Model and VAD artifacts

Columns: artifact ID, source repository, source model, immutable revision, SPDX, SHA-256, provenance.

<!-- BEGIN LIFESUB_ARTIFACT_NOTICES_V1 -->
qwen06-archive	https://github.com/k2-fsa/sherpa-onnx	sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2	github-release-asset:390698077	Apache-2.0	393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96	Sherpa release built from official Qwen3-ASR 0.6B using the linked Wasser1462 ONNX conversion.
qwen17-config	https://www.modelscope.cn/models/Qwen/Qwen3-ASR-1.7B	Qwen/Qwen3-ASR-1.7B	d69410f1c275f2b0fa60cbb9960edfcdb0ae0aec	Apache-2.0	2e74a751548b8ad7d7526d29365ad8144c345d8b412b1152d25dc6698452712f	Official original config with top-level thinker_config; conversion none.
qwen17-index	https://www.modelscope.cn/models/Qwen/Qwen3-ASR-1.7B	Qwen/Qwen3-ASR-1.7B	d69410f1c275f2b0fa60cbb9960edfcdb0ae0aec	Apache-2.0	f994739fe38e5210b9e3e8ce6c6307315e2ceac3cb630e7b7414d69dce520f60	Official original safetensors shard index; conversion none.
qwen17-tokenizer	https://huggingface.co/Qwen/Qwen3-ASR-1.7B-hf	Qwen/Qwen3-ASR-1.7B-hf	bcd2b5b7f32b480ab5790554cfa8347f246a14f3	Apache-2.0	fe1fad59be22a41ee293363fcf95fdedbc7c93f3b49270b1d2e18bd1399a7a05	Official -hf tokenizer mixed with official original config and weights; conversion none.
qwen17-weights-00001	https://www.modelscope.cn/models/Qwen/Qwen3-ASR-1.7B	Qwen/Qwen3-ASR-1.7B	d69410f1c275f2b0fa60cbb9960edfcdb0ae0aec	Apache-2.0	a4cd1f1a04d90b757dc7f7dd26254e69a013b19e80efe590a83c6a3bde8608d6	Official original safetensors weight shard 1; conversion none.
qwen17-weights-00002	https://www.modelscope.cn/models/Qwen/Qwen3-ASR-1.7B	Qwen/Qwen3-ASR-1.7B	d69410f1c275f2b0fa60cbb9960edfcdb0ae0aec	Apache-2.0	6e0b9d9e09e2e0238e7ef3cc8a484ab387e91b90f1900bedf88bc92d7929ccfc	Official original safetensors weight shard 2; conversion none.
sense-voice-archive	https://github.com/k2-fsa/sherpa-onnx	sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2	github-release-asset:288366523	LicenseRef-FunASR-Model-1.1	7d1efa2138a65b0b488df37f8b89e3d91a60676e416f515b952358d83dfd347e	Official sherpa-onnx conversion of FunAudioLLM SenseVoiceSmall INT8; archive LICENSE points to the FunASR model license.
silero-vad-onnx	https://github.com/k2-fsa/sherpa-onnx	silero_vad.onnx	github-release-asset:271935959	MIT	9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6	Silero VAD ONNX model distributed by sherpa-onnx; detector defaults are frozen from sherpa-onnx 1.13.5 source headers at commit 3dc7c569f31ca2cd4a20ed6f7db780327e6714c5.
whisper-base-archive	https://github.com/k2-fsa/sherpa-onnx	sherpa-onnx-whisper-base.tar.bz2	github-release-asset:196350768	MIT	911b2083efd7c0dca2ac3b358b75222660dc09fb716d64fbfc417ba6c99ff3de	Official sherpa-onnx ONNX export of OpenAI Whisper Base.
whisper-small-archive	https://github.com/k2-fsa/sherpa-onnx	sherpa-onnx-whisper-small.tar.bz2	github-release-asset:179373989	MIT	486a46afbb7ba798507190ffe02fea2dd726049af212e774537efac6afb210a6	Official sherpa-onnx ONNX export of OpenAI Whisper Small.
whisper-tiny-archive	https://github.com/k2-fsa/sherpa-onnx	sherpa-onnx-whisper-tiny.tar.bz2	github-release-asset:179373699	MIT	c46116994e539aa165266d96b325252728429c12535eb9d8b6a2b10f129e66b1	Official sherpa-onnx ONNX export of OpenAI Whisper Tiny.
<!-- END LIFESUB_ARTIFACT_NOTICES_V1 -->

## Qwen3-ASR Candle/Metal runtime closure

Columns: package, version, Cargo source, SPDX license expression, repository.

<!-- BEGIN LIFESUB_RUNTIME_CLOSURE_V1 -->
ahash	0.8.12	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/tkaitchuck/ahash
aho-corasick	1.1.5	registry+https://github.com/rust-lang/crates.io-index	Unlicense OR MIT	https://github.com/BurntSushi/aho-corasick
allocator-api2	0.2.21	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/zakarumych/allocator-api2
anyhow	1.0.104	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/anyhow
autocfg	1.5.1	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0 OR MIT	https://github.com/cuviper/autocfg
base64	0.13.1	registry+https://github.com/rust-lang/crates.io-index	MIT/Apache-2.0	https://github.com/marshallpierce/rust-base64
bitflags	1.3.2	registry+https://github.com/rust-lang/crates.io-index	MIT/Apache-2.0	https://github.com/bitflags/bitflags
bitflags	2.13.1	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/bitflags/bitflags
block	0.1.6	registry+https://github.com/rust-lang/crates.io-index	MIT	http://github.com/SSheldon/rust-block
block2	0.6.2	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/madsmtm/objc2
bumpalo	3.20.3	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/fitzgen/bumpalo
bytemuck	1.25.2	registry+https://github.com/rust-lang/crates.io-index	Zlib OR Apache-2.0 OR MIT	https://github.com/Lokathor/bytemuck
bytemuck_derive	1.12.0	registry+https://github.com/rust-lang/crates.io-index	Zlib OR Apache-2.0 OR MIT	https://github.com/Lokathor/bytemuck
byteorder	1.5.0	registry+https://github.com/rust-lang/crates.io-index	Unlicense OR MIT	https://github.com/BurntSushi/byteorder
candle-core	0.9.2	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/huggingface/candle
candle-metal-kernels	0.9.2	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/huggingface/candle
candle-nn	0.9.2	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/huggingface/candle
candle-ug	0.9.2	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/huggingface/candle
castaway	0.2.4	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sagebind/castaway
cc	1.4.3	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/cc-rs
cfg-if	1.0.4	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/cfg-if
compact_str	0.9.1	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/ParkMyCar/compact_str
console	0.16.4	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/console-rs/console
core-foundation	0.9.4	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/servo/core-foundation-rs
core-foundation-sys	0.8.7	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/servo/core-foundation-rs
core-graphics-types	0.1.3	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/servo/core-foundation-rs
crc32fast	1.5.0	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/srijs/rust-crc32fast
crossbeam-deque	0.8.7	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/crossbeam-rs/crossbeam
crossbeam-epoch	0.9.20	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/crossbeam-rs/crossbeam
crossbeam-utils	0.8.22	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/crossbeam-rs/crossbeam
crunchy	0.2.4	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/eira-fransham/crunchy
darling	0.20.11	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/TedDriggs/darling
darling_core	0.20.11	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/TedDriggs/darling
darling_macro	0.20.11	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/TedDriggs/darling
dary_heap	0.3.9	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/hanmertens/dary_heap
derive_builder	0.20.2	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/colin-kiegel/rust-derive-builder
derive_builder_core	0.20.2	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/colin-kiegel/rust-derive-builder
derive_builder_macro	0.20.2	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/colin-kiegel/rust-derive-builder
dispatch2	0.3.1	registry+https://github.com/rust-lang/crates.io-index	Zlib OR Apache-2.0 OR MIT	https://github.com/madsmtm/objc2
dyn-stack	0.13.2	registry+https://github.com/rust-lang/crates.io-index	MIT	https://codeberg.org/sarah-quinones/dyn-stack
dyn-stack-macros	0.1.3	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/kitegi/dynstack/
either	1.17.0	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rayon-rs/either
encode_unicode	1.0.0	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0 OR MIT	https://github.com/tormol/encode_unicode
enum-as-inner	0.6.1	registry+https://github.com/rust-lang/crates.io-index	MIT/Apache-2.0	https://github.com/bluejekyll/enum-as-inner
equivalent	1.0.2	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0 OR MIT	https://github.com/indexmap-rs/equivalent
esaxx-rs	0.1.10	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0	https://github.com/Narsil/esaxx-rs
find-msvc-tools	0.1.11	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/cc-rs
float8	0.6.1	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/EricLBuehler/float8
fnv	1.0.7	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0 / MIT	https://github.com/servo/rust-fnv
foldhash	0.2.0	registry+https://github.com/rust-lang/crates.io-index	Zlib	https://github.com/orlp/foldhash
foreign-types	0.5.0	registry+https://github.com/rust-lang/crates.io-index	MIT/Apache-2.0	https://github.com/sfackler/foreign-types
foreign-types-macros	0.2.4	registry+https://github.com/rust-lang/crates.io-index	MIT/Apache-2.0	https://github.com/sfackler/foreign-types
foreign-types-shared	0.3.1	registry+https://github.com/rust-lang/crates.io-index	MIT/Apache-2.0	https://github.com/sfackler/foreign-types
futures-core	0.3.34	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/futures-rs
futures-io	0.3.34	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/futures-rs
futures-macro	0.3.34	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/futures-rs
futures-sink	0.3.34	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/futures-rs
futures-task	0.3.34	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/futures-rs
futures-util	0.3.34	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/futures-rs
gemm	0.18.2	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/gemm/
gemm	0.19.0	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/gemm/
gemm-c32	0.18.2	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/gemm/
gemm-c32	0.19.0	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/gemm/
gemm-c64	0.18.2	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/gemm/
gemm-c64	0.19.0	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/gemm/
gemm-common	0.18.2	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/gemm/
gemm-common	0.19.0	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/gemm/
gemm-f16	0.18.2	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/gemm/
gemm-f16	0.19.0	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/gemm/
gemm-f32	0.18.2	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/gemm/
gemm-f32	0.19.0	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/gemm/
gemm-f64	0.18.2	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/gemm/
gemm-f64	0.19.0	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/gemm/
getrandom	0.3.4	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-random/getrandom
half	2.7.1	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/VoidStarKat/half-rs
hashbrown	0.16.1	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/hashbrown
hashbrown	0.17.1	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/hashbrown
heck	0.5.0	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/withoutboats/heck
hermit-abi	0.5.2	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/hermit-os/hermit-rs
hound	3.5.1	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0	https://github.com/ruuda/hound
ident_case	1.0.1	registry+https://github.com/rust-lang/crates.io-index	MIT/Apache-2.0	https://github.com/TedDriggs/ident_case
indexmap	2.14.0	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0 OR MIT	https://github.com/indexmap-rs/indexmap
indicatif	0.18.6	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/console-rs/indicatif
itertools	0.14.0	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-itertools/itertools
itoa	1.0.18	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/itoa
js-sys	0.3.104	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/js-sys
libc	0.2.189	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/libc
libloading	0.8.9	registry+https://github.com/rust-lang/crates.io-index	ISC	https://github.com/nagisa/rust_libloading/
libm	0.2.16	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/rust-lang/compiler-builtins
log	0.4.33	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/log
macro_rules_attribute	0.2.3	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0 OR MIT OR Zlib	https://github.com/danielhenrymantilla/macro_rules_attribute-rs
macro_rules_attribute-proc_macro	0.2.3	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0 OR MIT OR Zlib	https://github.com/danielhenrymantilla/macro_rules_attribute-rs
malloc_buf	0.0.6	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/SSheldon/malloc_buf
memchr	2.8.3	registry+https://github.com/rust-lang/crates.io-index	Unlicense OR MIT	https://github.com/BurntSushi/memchr
memmap2	0.9.11	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/RazrFalcon/memmap2-rs
metal	0.29.0	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/gfx-rs/metal-rs
minimal-lexical	0.2.1	registry+https://github.com/rust-lang/crates.io-index	MIT/Apache-2.0	https://github.com/Alexhuszagh/minimal-lexical
monostate	0.1.18	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/monostate
monostate-impl	0.1.18	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/monostate
nom	7.1.3	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/Geal/nom
num	0.4.3	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-num/num
num-bigint	0.4.8	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-num/num-bigint
num-complex	0.4.6	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-num/num-complex
num-integer	0.1.47	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-num/num-integer
num-iter	0.1.46	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-num/num-iter
num-rational	0.4.2	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-num/num-rational
num-traits	0.2.19	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-num/num-traits
num_cpus	1.17.0	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/seanmonstar/num_cpus
objc	0.2.7	registry+https://github.com/rust-lang/crates.io-index	MIT	http://github.com/SSheldon/rust-objc
objc2	0.6.4	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/madsmtm/objc2
objc2-core-foundation	0.3.2	registry+https://github.com/rust-lang/crates.io-index	Zlib OR Apache-2.0 OR MIT	https://github.com/madsmtm/objc2
objc2-encode	4.1.0	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/madsmtm/objc2
objc2-exception-helper	0.1.1	registry+https://github.com/rust-lang/crates.io-index	Zlib OR Apache-2.0 OR MIT	https://github.com/madsmtm/objc2
objc2-foundation	0.3.2	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/madsmtm/objc2
objc2-metal	0.3.2	registry+https://github.com/rust-lang/crates.io-index	Zlib OR Apache-2.0 OR MIT	https://github.com/madsmtm/objc2
once_cell	1.21.4	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/matklad/once_cell
onig	6.5.3	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/iwillspeak/rust-onig
onig_sys	69.9.3	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/rust-onig/rust-onig
paste	1.0.15	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/paste
pastey	0.2.3	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/as1100k/pastey
pin-project-lite	0.2.17	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0 OR MIT	https://github.com/taiki-e/pin-project-lite
pkg-config	0.3.34	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/pkg-config-rs
portable-atomic	1.15.0	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0 OR MIT	https://github.com/taiki-e/portable-atomic
ppv-lite86	0.2.21	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/cryptocorrosion/cryptocorrosion
primal-check	0.3.4	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/huonw/primal
proc-macro2	1.0.107	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/proc-macro2
pulp	0.21.5	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/pulp/
pulp	0.22.3	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-quinones/pulp/
pulp-wasm-simd-flag	0.1.1	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-quinones/pulp/
quote	1.0.47	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/quote
qwen3-asr	0.2.2	git+https://github.com/alan890104/qwen3-asr-rs.git?rev=c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc#c5ef09646af6278d2ba8b8ceaf543ffb32d1a5dc	MIT	https://github.com/alan890104/qwen3-asr-rs
r-efi	5.3.0	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0 OR LGPL-2.1-or-later	https://github.com/r-efi/r-efi
rand	0.9.5	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-random/rand
rand_chacha	0.9.0	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-random/rand
rand_core	0.9.5	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-random/rand
rand_distr	0.5.1	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-random/rand_distr
raw-cpuid	11.6.0	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/gz/rust-cpuid
rayon	1.12.0	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rayon-rs/rayon
rayon-cond	0.4.0	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0/MIT	https://github.com/cuviper/rayon-cond
rayon-core	1.13.0	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rayon-rs/rayon
realfft	3.5.0	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/HEnquist/realfft
reborrow	0.5.5	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/sarah-ek/reborrow/
regex	1.13.1	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/regex
regex-automata	0.4.18	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/regex
regex-syntax	0.8.11	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/rust-lang/regex
rubato	0.15.0	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/HEnquist/rubato
rustfft	6.4.1	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/ejmahler/RustFFT
rustversion	1.0.23	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/rustversion
ryu	1.0.23	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0 OR BSL-1.0	https://github.com/dtolnay/ryu
safetensors	0.4.5	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0	https://github.com/huggingface/safetensors
safetensors	0.7.0	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0	https://github.com/huggingface/safetensors
same-file	1.0.6	registry+https://github.com/rust-lang/crates.io-index	Unlicense/MIT	https://github.com/BurntSushi/same-file
seq-macro	0.3.6	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/seq-macro
serde	1.0.229	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/serde-rs/serde
serde_core	1.0.229	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/serde-rs/serde
serde_derive	1.0.229	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/serde-rs/serde
serde_json	1.0.151	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/serde-rs/json
shlex	2.0.1	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/comex/rust-shlex
slab	0.4.12	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/tokio-rs/slab
smallvec	1.15.2	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/servo/rust-smallvec
spm_precompiled	0.1.4	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0	https://github.com/huggingface/spm_precompiled
stable_deref_trait	1.2.1	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/storyyeller/stable_deref_trait
static_assertions	1.1.0	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/nvzqz/static-assertions-rs
strength_reduce	0.2.4	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	http://github.com/ejmahler/strength_reduce
strsim	0.11.1	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/rapidfuzz/strsim-rs
syn	2.0.119	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/syn
syn	3.0.3	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/syn
synstructure	0.13.2	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/mystor/synstructure
sysctl	0.6.0	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/johalun/sysctl-rs
thiserror	1.0.69	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/thiserror
thiserror	2.0.20	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/thiserror
thiserror-impl	1.0.69	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/thiserror
thiserror-impl	2.0.20	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/dtolnay/thiserror
tokenizers	0.22.2	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0	https://github.com/huggingface/tokenizers
tracing	0.1.44	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/tokio-rs/tracing
tracing-attributes	0.1.31	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/tokio-rs/tracing
tracing-core	0.1.36	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/tokio-rs/tracing
transpose	0.2.3	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/ejmahler/transpose
typed-path	0.12.3	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/chipsenkbeil/typed-path
ug	0.5.0	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/LaurentMazare/ug
ug-metal	0.5.0	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/LaurentMazare/ug
unicode-ident	1.0.24	registry+https://github.com/rust-lang/crates.io-index	(MIT OR Apache-2.0) AND Unicode-3.0	https://github.com/dtolnay/unicode-ident
unicode-normalization-alignments	0.1.12	registry+https://github.com/rust-lang/crates.io-index	MIT/Apache-2.0	https://github.com/n1t0/unicode-normalization
unicode-segmentation	1.13.3	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/unicode-rs/unicode-segmentation
unicode-width	0.2.2	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/unicode-rs/unicode-width
unicode_categories	0.1.1	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/swgillespie/unicode-categories
unit-prefix	0.5.2	registry+https://github.com/rust-lang/crates.io-index	MIT	https://codeberg.org/commons-rs/unit-prefix
version_check	0.9.5	registry+https://github.com/rust-lang/crates.io-index	MIT/Apache-2.0	https://github.com/SergioBenitez/version_check
walkdir	2.5.0	registry+https://github.com/rust-lang/crates.io-index	Unlicense/MIT	https://github.com/BurntSushi/walkdir
wasip2	1.0.4+wasi-0.2.12	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT	https://github.com/bytecodealliance/wasi-rs
wasm-bindgen	0.2.127	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/wasm-bindgen/wasm-bindgen
wasm-bindgen-macro	0.2.127	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/macro
wasm-bindgen-macro-support	0.2.127	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/macro-support
wasm-bindgen-shared	0.2.127	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/shared
web-time	1.1.0	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/daxpedda/web-time
winapi-util	0.1.11	registry+https://github.com/rust-lang/crates.io-index	Unlicense OR MIT	https://github.com/BurntSushi/winapi-util
windows-link	0.2.1	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/microsoft/windows-rs
windows-sys	0.61.2	registry+https://github.com/rust-lang/crates.io-index	MIT OR Apache-2.0	https://github.com/microsoft/windows-rs
wit-bindgen	0.57.1	registry+https://github.com/rust-lang/crates.io-index	Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT	https://github.com/bytecodealliance/wit-bindgen
yoke	0.7.5	registry+https://github.com/rust-lang/crates.io-index	Unicode-3.0	https://github.com/unicode-org/icu4x
yoke	0.8.3	registry+https://github.com/rust-lang/crates.io-index	Unicode-3.0	https://github.com/unicode-org/icu4x
yoke-derive	0.7.5	registry+https://github.com/rust-lang/crates.io-index	Unicode-3.0	https://github.com/unicode-org/icu4x
yoke-derive	0.8.2	registry+https://github.com/rust-lang/crates.io-index	Unicode-3.0	https://github.com/unicode-org/icu4x
zerocopy	0.8.56	registry+https://github.com/rust-lang/crates.io-index	BSD-2-Clause OR Apache-2.0 OR MIT	https://github.com/google/zerocopy
zerocopy-derive	0.8.56	registry+https://github.com/rust-lang/crates.io-index	BSD-2-Clause OR Apache-2.0 OR MIT	https://github.com/google/zerocopy
zerofrom	0.1.8	registry+https://github.com/rust-lang/crates.io-index	Unicode-3.0	https://github.com/unicode-org/icu4x
zerofrom-derive	0.1.7	registry+https://github.com/rust-lang/crates.io-index	Unicode-3.0	https://github.com/unicode-org/icu4x
zip	7.2.0	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/zip-rs/zip2.git
zmij	1.0.23	registry+https://github.com/rust-lang/crates.io-index	MIT	https://github.com/dtolnay/zmij
<!-- END LIFESUB_RUNTIME_CLOSURE_V1 -->
