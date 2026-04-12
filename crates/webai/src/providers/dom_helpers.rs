/// Build JS that sends a message via DOM interaction and polls for the response.
///
/// `input_selectors`: CSS selectors to find the chat input (tried in order).
/// `response_extract_js`: JS expression that returns the latest assistant text (string).
/// `poll_interval_ms`: polling interval.
/// `max_wait_ms`: max time to wait for response.
/// `stability_threshold`: how many stable reads before considering done.
pub fn build_dom_send_js(
    message: &str,
    input_selectors: &[&str],
    response_extract_js: &str,
    poll_interval_ms: u32,
    max_wait_ms: u32,
    stability_threshold: u32,
) -> String {
    let msg_escaped = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".into());
    let selectors_js: Vec<String> = input_selectors
        .iter()
        .map(|s| format!("'{}'", s.replace('\'', "\\'")))
        .collect();
    let selectors_array = selectors_js.join(", ");

    format!(
        r#"
const message = {msg_escaped};
const selectors = [{selectors_array}];
let inputEl = null;
for (const sel of selectors) {{
    inputEl = document.querySelector(sel);
    if (inputEl) break;
}}
if (!inputEl) throw new Error('DOM send: chat input not found');

inputEl.focus();
inputEl.click();
await new Promise(r => setTimeout(r, 300));

if (inputEl.tagName === 'TEXTAREA' || inputEl.tagName === 'INPUT') {{
    const nativeSet = Object.getOwnPropertyDescriptor(
        window.HTMLTextAreaElement?.prototype || window.HTMLInputElement?.prototype,
        'value'
    )?.set || Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')?.set;
    if (nativeSet) nativeSet.call(inputEl, message);
    else inputEl.value = message;
    inputEl.dispatchEvent(new Event('input', {{ bubbles: true }}));
}} else {{
    inputEl.innerText = message;
    inputEl.dispatchEvent(new Event('input', {{ bubbles: true }}));
}}
await new Promise(r => setTimeout(r, 300));

inputEl.dispatchEvent(new KeyboardEvent('keydown', {{ key: 'Enter', code: 'Enter', keyCode: 13, bubbles: true }}));
inputEl.dispatchEvent(new KeyboardEvent('keypress', {{ key: 'Enter', code: 'Enter', keyCode: 13, bubbles: true }}));
inputEl.dispatchEvent(new KeyboardEvent('keyup', {{ key: 'Enter', code: 'Enter', keyCode: 13, bubbles: true }}));

const pollInterval = {poll_interval_ms};
const maxWait = {max_wait_ms};
const threshold = {stability_threshold};
let lastText = '';
let stableCount = 0;

for (let elapsed = 0; elapsed < maxWait; elapsed += pollInterval) {{
    await new Promise(r => setTimeout(r, pollInterval));
    const text = (() => {{ {response_extract_js} }})();
    if (text && text.length >= 2) {{
        if (text !== lastText) {{
            lastText = text;
            stableCount = 0;
        }} else {{
            stableCount++;
            if (stableCount >= threshold) break;
        }}
    }}
}}

if (!lastText) throw new Error('DOM send: no assistant reply detected');
return lastText;
"#
    )
}
