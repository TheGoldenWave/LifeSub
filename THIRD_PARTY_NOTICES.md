# Third-Party Notices

LifeSub includes or depends on the following third-party software components.

## Model Weights

The following pre-trained model weights are downloaded by the user at runtime
from the sources listed below. They are not distributed with the LifeSub
application binary.

### SenseVoiceSmall

- **Model:** SenseVoiceSmall (INT8 quantized ONNX)
- **Upstream:** Alibaba FunASR / Modelscope
- **Repository:** https://github.com/modelscope/FunASR
- **Original Model ID:** iic/SenseVoiceSmall
- **License:** MIT
- **License URL:** https://github.com/modelscope/FunASR/blob/main/LICENSE
- **Converted By:** k2-fsa/sherpa-onnx
- **Conversion Repository:** https://github.com/k2-fsa/sherpa-onnx
- **Download URL:** https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2
- **SHA-256:** `7d1efa2138a65b0b488df37f8b89e3d91a60676e416f515b952358d83dfd347e`

### Whisper (Tiny, Base, Small)

- **Model:** OpenAI Whisper (Tiny, Base, Small) ONNX
- **Upstream:** OpenAI
- **Repository:** https://github.com/openai/whisper
- **License:** MIT
- **License URL:** https://github.com/openai/whisper/blob/main/LICENSE
- **Converted By:** k2-fsa/sherpa-onnx
- **Conversion Repository:** https://github.com/k2-fsa/sherpa-onnx

#### Whisper Tiny

- **Download URL:** https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-tiny.tar.bz2
- **SHA-256:** `c46116994e539aa165266d96b325252728429c12535eb9d8b6a2b10f129e66b1`

#### Whisper Base

- **Download URL:** https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.tar.bz2
- **SHA-256:** `911b2083efd7c0dca2ac3b358b75222660dc09fb716d64fbfc417ba6c99ff3de`

#### Whisper Small

- **Download URL:** https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-small.tar.bz2
- **SHA-256:** (pending download verification — network timeout on 610 MB file)

### Silero VAD

- **Model:** Silero Voice Activity Detection (VAD)
- **Upstream:** Silero
- **Repository:** https://github.com/snakers4/silero-vad
- **License:** MIT
- **License URL:** https://github.com/snakers4/silero-vad/blob/master/LICENSE
- **Converted By:** k2-fsa/sherpa-onnx
- **Conversion Repository:** https://github.com/k2-fsa/sherpa-onnx
- **Download URL:** https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx
- **SHA-256:** `9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6`

## Runtime Libraries

### sherpa-onnx

- **Version:** 1.13.5
- **Commit:** 3dc7c569f31ca2cd4a20ed6f7db780327e6714c5
- **Repository:** https://github.com/k2-fsa/sherpa-onnx
- **License:** Apache-2.0
- **License URL:** https://github.com/k2-fsa/sherpa-onnx/blob/master/LICENSE
- **Usage:** Statically linked ASR runtime for SenseVoice and Whisper inference.

## Additional Dependencies

For a complete list of Rust crate dependencies and their licenses, see
`src-tauri/Cargo.lock` and run `cargo license` in the `src-tauri` directory.

For npm dependencies, see `package.json` and run `npx license-checker`.

---

*This file is maintained as part of the LifeSub project. If you believe any
license information is incorrect, please open an issue.*