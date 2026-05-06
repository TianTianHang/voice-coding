## MODIFIED Requirements

### Requirement: Create ONNX inference sessions

The system SHALL create and manage ONNX Runtime inference sessions for all Qwen3-ASR models from the model directory resolved by unified model path management.

#### Scenario: Session creation

- **WHEN** initializing ONNX sessions
- **THEN** it SHALL create one session per required Qwen3 ASR ONNX model file
- **AND** it SHALL use CPU execution provider
- **AND** it SHALL enable all graph optimization levels
- **AND** it SHALL set intra-op num threads if specified

#### Scenario: Session options configuration

- **WHEN** configuring session options
- **THEN** graph optimization level SHALL be ORT_ENABLE_ALL
- **AND** execution mode SHALL be sequential
- **AND** log severity level SHALL be 3 (suppress warnings)
- **AND** it MAY set execution mode to parallel if beneficial

#### Scenario: Model file paths

- **WHEN** loading Qwen3 ASR model files from the resolved ASR model directory `{model_dir}`
- **THEN** encoder SHALL be loaded from either `{model_dir}/onnx_models/encoder.int4.onnx` or `{model_dir}/onnx_models/encoder.onnx`
- **AND** decoder_init SHALL be loaded from either `{model_dir}/onnx_models/decoder_init.int4.onnx` or `{model_dir}/onnx_models/decoder_init.onnx`
- **AND** decoder_step SHALL be loaded from either `{model_dir}/onnx_models/decoder_step.int4.onnx` or `{model_dir}/onnx_models/decoder_step.onnx`

#### Scenario: Standard model package path

- **WHEN** unified model path management resolves Qwen3 ASR to the standard layout
- **THEN** `{model_dir}` SHALL be `<model-home>/asr/qwen3-asr-0.6b-onnx`
- **AND** ONNX sessions SHALL be loaded from `<model-home>/asr/qwen3-asr-0.6b-onnx/onnx_models`

#### Scenario: Legacy model package path

- **WHEN** unified model path management resolves Qwen3 ASR to the legacy development layout
- **THEN** `{model_dir}` SHALL be `./models`
- **AND** ONNX sessions SHALL continue to load from `./models/onnx_models`

#### Scenario: Session creation failure

- **WHEN** model file is missing or corrupted
- **THEN** system SHALL return `InferenceError`
- **AND** error SHALL specify which model failed to load
- **AND** error SHALL include the resolved path that failed validation
