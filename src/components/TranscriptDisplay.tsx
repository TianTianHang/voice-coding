interface TranscriptDisplayProps {
  text: string;
  error: string | null;
}

export function TranscriptDisplay({ text, error }: TranscriptDisplayProps) {
  if (error) {
    return (
      <div style={{ padding: 16, backgroundColor: "#ffeaea", borderRadius: 8, color: "#c0392b" }}>
        Transcription error: {error}
      </div>
    );
  }

  if (!text) {
    return (
      <div style={{ padding: 16, color: "#888", fontStyle: "italic" }}>
        Transcribed text will appear here...
      </div>
    );
  }

  return (
    <div style={{ padding: 16, backgroundColor: "#f0f7ff", borderRadius: 8, whiteSpace: "pre-wrap", lineHeight: 1.6 }}>
      {text}
    </div>
  );
}
