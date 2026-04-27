## MODIFIED Requirements

### Requirement: Greedy token decoding

The system SHALL implement greedy decoding to select most likely tokens at each step.

#### Scenario: Select next token

- **WHEN** decoder produces logits
- **THEN** system SHALL extract last logits vector: `logits[0, -1, :]`
- **AND** it SHALL compute argmax: `next_token = argmax(logits)`
- **AND** next_token SHALL be u32 scalar
- **AND** it SHALL NOT require copying the full logits tensor into a new owned array before argmax

#### Scenario: Append to sequence

- **WHEN** next token is selected
- **THEN** it SHALL append to generated tokens list
- **AND** it SHALL increment position counter
- **AND** it SHALL continue until stop condition

#### Scenario: Stop conditions

- **WHEN** checking stop conditions
- **THEN** it SHALL stop if token == IM_END_ID (151645)
- **AND** it SHALL stop if token == ENDOFTEXT_ID (151643)
- **AND** it SHALL stop if generated tokens reach max_new_tokens
- **AND** stop tokens SHALL NOT be included in final output
