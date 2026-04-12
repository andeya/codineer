/// Incremental SSE line parser that handles partial chunks.
///
/// Feed raw text chunks (from WebView streaming) via [`push`], then
/// drain parsed SSE `data:` payloads with [`drain_events`].
pub struct SseLineParser {
    buffer: String,
}

impl SseLineParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Append a raw text chunk (may contain partial lines).
    pub fn push(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);
    }

    /// Drain all complete `data: ...` payloads from the buffer.
    /// Incomplete lines are kept for the next `push`.
    pub fn drain_events(&mut self) -> Vec<String> {
        let mut events = Vec::new();
        let mut consumed = 0;
        while let Some(rel) = self.buffer[consumed..].find('\n') {
            let line = self.buffer[consumed..consumed + rel].trim();
            consumed += rel + 1;
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                if !data.is_empty() && data != "[DONE]" {
                    events.push(data.to_string());
                }
            }
        }
        if consumed > 0 {
            self.buffer.drain(..consumed);
        }
        events
    }

    /// Flush any remaining buffered content as a final event.
    pub fn flush(&mut self) -> Vec<String> {
        let remaining = std::mem::take(&mut self.buffer);
        let trimmed = remaining.trim();
        if let Some(data) = trimmed.strip_prefix("data:") {
            let data = data.trim();
            if !data.is_empty() && data != "[DONE]" {
                return vec![data.to_string()];
            }
        }
        Vec::new()
    }
}

impl Default for SseLineParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_sse_parsing() {
        let mut parser = SseLineParser::new();
        parser.push("data: {\"text\":\"hello\"}\n\ndata: {\"text\":\"world\"}\n\n");
        let events = parser.drain_events();
        assert_eq!(events, vec!["{\"text\":\"hello\"}", "{\"text\":\"world\"}"]);
    }

    #[test]
    fn test_partial_chunks() {
        let mut parser = SseLineParser::new();
        parser.push("data: {\"te");
        assert!(parser.drain_events().is_empty());

        parser.push("xt\":\"hi\"}\n\n");
        let events = parser.drain_events();
        assert_eq!(events, vec!["{\"text\":\"hi\"}"]);
    }

    #[test]
    fn test_done_filtered() {
        let mut parser = SseLineParser::new();
        parser.push("data: {\"a\":1}\ndata: [DONE]\n");
        let events = parser.drain_events();
        assert_eq!(events, vec!["{\"a\":1}"]);
    }
}
