# Spec: Audio Preprocessing

## ADDED Requirements

### Requirement: Load audio from file with format detection

The system SHALL load audio files using Symphonia with automatic format detection and convert to required format.

#### Scenario: Support common audio formats

- **WHEN** loading audio file with extension .wav, .mp3, .flac, .ogg, or .m4a
- **THEN** system SHALL detect format from file header
- **AND** it SHALL decode audio data successfully

#### Scenario: Convert to 16kHz mono float32

- **WHEN** loading any supported audio file
- **THEN** output sample rate SHALL be exactly 16000 Hz
- **AND** output channels SHALL be 1 (mono)
- **AND** output data type SHALL be f32 (floating point)
- **AND** if input is stereo, it SHALL downmix to mono (average channels)
- **AND** if input sample rate differs, it SHALL resample to 16kHz

#### Scenario: Handle variable bitrates and encodings

- **WHEN** loading compressed audio (MP3, OGG, M4A)
- **THEN** system SHALL decode regardless of bitrate
- **AND** it SHALL handle different encodings (AAC, Vorbis, MP3)

#### Scenario: File not found or corrupted

- **WHEN** audio file does not exist or is corrupted
- **THEN** system SHALL return `AudioLoadError`
- **AND** error message SHALL specify file path and reason

### Requirement: Load audio from byte buffer

The system SHALL decode audio from in-memory byte buffers without temporary files.

#### Scenario: Byte buffer input

- **WHEN** audio is provided as `Vec<u8>` bytes
- **THEN** system SHALL use Symphonia's `Cursor` to read from memory
- **AND** it SHALL detect format from magic bytes in buffer
- **AND** it MUST NOT create temporary files on disk

#### Scenario: Format detection from bytes

- **WHEN** byte buffer contains WAV/MP3/FLAC data
- **THEN** system SHALL identify format from header bytes
- **AND** it SHALL decode without requiring file extension

#### Scenario: Invalid byte buffer

- **WHEN** byte buffer does not contain valid audio data
- **THEN** system SHALL return `AudioLoadError`
- **AND** error SHALL indicate unsupported or corrupted data

### Requirement: Accept raw audio samples

The system SHALL accept pre-decoded float32 samples for advanced use cases.

#### Scenario: Raw samples at 16kHz

- **WHEN** user provides `AudioInput::Samples(vec, 16000)`
- **THEN** system SHALL use samples directly without decoding
- **AND** it SHALL skip format detection and resampling

#### Scenario: Raw samples at different sample rate

- **WHEN** user provides samples at 48000 Hz
- **THEN** system SHALL resample to 16kHz
- **AND** it SHALL apply high-quality resampling algorithm

#### Scenario: Validate sample rate

- **WHEN** user provides samples at unsupported rate (< 8kHz or > 48kHz)
- **THEN** system SHALL return `AudioLoadError`
- **AND** error SHALL specify valid sample rate range

### Requirement: Validate audio duration

The system SHALL enforce minimum and maximum audio duration limits.

#### Scenario: Minimum duration

- **WHEN** audio duration is less than 0.1 seconds
- **THEN** system SHALL return `AudioLoadError`
- **AND** error SHALL specify minimum 0.1s requirement (reason: too short for meaningful transcription)

#### Scenario: Maximum duration without chunking

- **WHEN** audio duration exceeds 45 seconds and VAD is disabled
- **THEN** system SHALL emit warning (but still process)
- **AND** it MAY return error if duration would cause OOM

#### Scenario: No duration limit with chunking

- **WHEN** VAD chunking is enabled
- **THEN** system SHALL accept arbitrarily long audio
- **AND** it SHALL split into manageable chunks automatically

### Requirement: Compute Mel spectrogram

The system SHALL compute log-Mel spectrogram from audio samples matching librosa's implementation.

#### Scenario: STFT parameters

- **WHEN** computing Short-Time Fourier Transform
- **THEN** window function SHALL be Hann window
- **AND** n_fft (window size) SHALL be 400 samples (25ms at 16kHz)
- **AND** hop_length (step size) SHALL be 160 samples (10ms at 16kHz)
- **AND** window SHALL be centered (default behavior)
- **AND** pad mode for edges SHALL be "reflect"

#### Scenario: STFT computation

- **WHEN** applying STFT to audio samples
- **THEN** it SHALL compute complex-valued DFT for each frame
- **AND** output shape SHALL be `[n_fft // 2 + 1, n_frames]` = `[201, n_frames]`
- **AND** n_frames SHALL equal `(samples - n_fft) // hop_length + 1`

#### Scenario: Magnitude spectrogram

- **WHEN** converting complex STFT to magnitude
- **THEN** it SHALL compute magnitude: `magnitude = sqrt(real^2 + imag^2)`
- **AND** it SHALL square magnitude: `power = magnitude^2`
- **AND** output SHALL be real-valued float32

### Requirement: Mel filterbank application

The system SHALL apply Mel-scale filterbank to magnitude spectrogram.

#### Scenario: Mel filterbank parameters

- **WHEN** creating Mel filterbank
- **THEN** number of Mel bins SHALL be 128
- **AND** minimum frequency SHALL be 0 Hz
- **AND** maximum frequency SHALL be 8000 Hz (Nyquist at 16kHz)
- **AND** normalization type SHALL be "slaney" (area normalization)
- **AND** htk parameter SHALL be false (use Slaney mel scale)

#### Scenario: Filterbank matrix

- **WHEN** generating Mel filterbank matrix
- **THEN** shape SHALL be `[n_mels, n_fft // 2 + 1]` = `[128, 201]`
- **AND** each row SHALL be a triangular filter in Mel frequency space
- **AND** filters SHALL be non-overlapping at peak frequencies
- **AND** matrix SHALL be pre-computed and cached

#### Scenario: Apply filterbank

- **WHEN** multiplying filterbank with magnitude spectrogram
- **THEN** it SHALL compute matrix product: `mel_spec = filterbank @ power_spec`
- **AND** output shape SHALL be `[128, n_frames]`
- **AND** dtype SHALL be float32

### Requirement: Log compression and normalization

The system SHALL apply logarithmic compression and normalize to model input range.

#### Scenario: Log magnitude

- **WHEN** computing log spectrogram
- **THEN** formula SHALL be: `log_spec = log10(max(mel_spec, 1e-10))`
- **AND** floor value (1e-10) prevents log(0)
- **AND** output SHALL be in decibel-like scale

#### Scenario: Dynamic range compression

- **WHEN** compressing dynamic range
- **THEN** it SHALL clip to max-8.0: `log_spec = max(log_spec, log_spec.max() - 8.0)`
- **AND** this keeps only top 8 decibels of energy

#### Scenario: Normalize to [-1, 1]

- **WHEN** normalizing log spectrogram
- **THEN** formula SHALL be: `normalized = (log_spec + 4.0) / 4.0`
- **AND** output range SHALL be approximately [-1, 1]
- **AND** output dtype SHALL be float32

### Requirement: Numerical accuracy

The system SHALL match librosa's output within acceptable tolerance for model compatibility.

#### Scenario: Bit-exact filterbank

- **WHEN** generating Mel filterbank
- **THEN** coefficients SHALL match librosa within 1e-6 tolerance
- **AND** test suite SHALL validate against reference values

#### Scenario: STFT compatibility

- **WHEN** computing STFT with same parameters as librosa
- **THEN** output SHALL match librosa within 1e-5 tolerance
- **AND** differences SHALL be due to floating point rounding only

#### Scenario: End-to-end Mel comparison

- **WHEN** computing full Mel spectrogram
- **THEN** output SHALL match librosa.melspectrogram within 1e-4 tolerance
- **AND** mean absolute error SHALL be < 0.001

### Requirement: Performance optimization

The system SHALL optimize Mel computation for real-time performance.

#### Scenario: Pre-computed filterbank

- **WHEN** engine initializes
- **THEN** Mel filterbank SHALL be computed once and cached
- **AND** subsequent calls SHALL reuse cached matrix

#### Scenario: Efficient STFT

- **WHEN** computing STFT
- **THEN** it SHALL use RustFFT for optimized FFT computation
- **AND** it MAY use SIMD instructions if available

#### Scenario: Memory allocation

- **WHEN** computing Mel spectrogram
- **THEN** it SHALL pre-allocate output buffers
- **AND** it SHALL avoid unnecessary allocations in hot loop

### Requirement: VAD-based audio splitting

The system SHALL detect silence boundaries for splitting long audio files.

#### Scenario: RMS energy computation

- **WHEN** computing audio energy for VAD
- **THEN** it SHALL compute RMS (root-mean-square) energy
- **AND** frame length SHALL be 2 × hop = 0.2 seconds
- **AND** hop size SHALL be 0.1 seconds
- **AND** output SHALL be energy per frame

#### Scenario: Convert to dB

- **WHEN** converting RMS to decibels
- **THEN** formula SHALL be: `rms_db = 20 * log10(rms / max(rms))`
- **AND** reference SHALL be peak RMS in entire audio

#### Scenario: Silence detection

- **WHEN** detecting silent frames
- **THEN** frame SHALL be silent if `rms_db < -40` (threshold)
- **AND** it SHALL output boolean mask of same length as RMS

#### Scenario: Find split points

- **WHEN** finding optimal split points for chunking
- **THEN** search range SHALL be [target/2, target×1.5] seconds
- **AND** it SHALL find silent frame nearest to target duration
- **AND** if no silence in range, it SHALL split at target anyway
- **AND** output SHALL be list of sample indices for splitting

#### Scenario: Split at boundaries

- **WHEN** splitting audio at detected points
- **THEN** it SHALL create sub-chunks: audio[0:split1], audio[split1:split2], ...
- **AND** each chunk SHALL be processed independently
- **AND** results SHALL be concatenated with spaces

### Requirement: Error handling for audio preprocessing

The system SHALL provide clear error messages for audio preprocessing failures.

#### Scenario: Unsupported codec

- **WHEN** audio file uses unsupported codec (e.g., DSD, proprietary)
- **THEN** system SHALL return `AudioLoadError`
- **AND** error SHALL list supported formats

#### Scenario: Resampling failure

- **WHEN** resampling fails (e.g., extreme ratio)
- **THEN** system SHALL return `AudioLoadError`
- **AND** error SHALL specify input and target sample rates

#### Scenario: Mel computation failure

- **WHEN** Mel spectrogram computation fails (e.g., empty audio)
- **THEN** system SHALL return `AudioLoadError`
- **AND** error SHALL indicate computation step that failed

#### Scenario: Invalid audio parameters

- **WHEN** audio has invalid parameters (e.g., NaN samples, infinite duration)
- **THEN** system SHALL return `AudioLoadError`
- **AND** error SHALL describe which parameter is invalid
