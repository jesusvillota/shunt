use super::tool_protocol::{Contract, Parser};
use serde_json::{json, Value};

#[derive(serde::Serialize)]
struct Message {
    role: String,
    content: String,
}

pub(super) fn prompt(request: &Value) -> Result<String, &'static str> {
    if request
        .get("tools")
        .is_some_and(|tools| !tools.as_array().is_some_and(Vec::is_empty))
    {
        return Err("tools must be a supported declaration array");
    }
    if request
        .get("previous_response_id")
        .is_some_and(|v| !v.is_null())
    {
        return Err(
            "chatgptweb_browser needs the full transcript; previous_response_id is unsupported",
        );
    }
    let mut transcript = Vec::new();
    if let Some(instructions) = request.get("instructions").filter(|v| !v.is_null()) {
        transcript.push(Message {
            role: "system".into(),
            content: instructions
                .as_str()
                .ok_or("instructions must be text")?
                .into(),
        });
    }
    match &request["input"] {
        Value::String(text) => transcript.push(Message {
            role: "user".into(),
            content: text.clone(),
        }),
        Value::Array(messages) => {
            for message in messages {
                if message.get("type").is_some_and(|v| v != "message") {
                    return Err("chatgptweb_browser accepts text messages only");
                }
                let role = message["role"]
                    .as_str()
                    .filter(|role| matches!(*role, "system" | "developer" | "user" | "assistant"))
                    .ok_or("unsupported message role")?;
                let text = match &message["content"] {
                    Value::String(text) => text.clone(),
                    Value::Array(parts) => {
                        let mut text = String::new();
                        for part in parts {
                            if !matches!(part["type"].as_str(), Some("input_text" | "output_text"))
                            {
                                return Err("chatgptweb_browser accepts text content only");
                            }
                            text.push_str(
                                part["text"]
                                    .as_str()
                                    .ok_or("content text must be a string")?,
                            );
                        }
                        text
                    }
                    _ => return Err("message content must be text"),
                };
                transcript.push(Message {
                    role: role.into(),
                    content: text,
                });
            }
        }
        _ => return Err("input must be text or a message array"),
    }
    if transcript.is_empty() {
        return Err("input must contain a text message");
    }
    Ok(format!(
        "Continue this conversation. Return only the assistant's next text answer.\n{}",
        serde_json::to_string(&transcript).unwrap()
    ))
}

pub(super) struct Output {
    pub id: String,
    model: String,
    item: String,
    text: String,
    sequence: u64,
    created_at: u64,
    parser: Option<Parser>,
    calls: Vec<Value>,
    tools: Value,
    choice: Value,
}
impl Output {
    pub fn new(id: String, model: String, contract: Option<Contract>, request: &Value) -> Self {
        Self {
            item: format!("msg_{}", uuid::Uuid::new_v4().simple()),
            id,
            model,
            parser: contract.map(Parser::new),
            calls: Vec::new(),
            tools: request.get("tools").cloned().unwrap_or(json!([])),
            choice: request.get("tool_choice").cloned().unwrap_or(json!("auto")),
            text: String::new(),
            sequence: 0,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    fn frame(&mut self, kind: &str, mut value: Value) -> String {
        value["type"] = json!(kind);
        value["sequence_number"] = json!(self.sequence);
        self.sequence += 1;
        format!("event: {kind}\ndata: {value}\n\n")
    }
    fn part(&self, done: bool) -> Value {
        json!({"type":"output_text","text":if done { &self.text } else { "" },"annotations":[],"logprobs":[]})
    }
    fn item(&self, done: bool) -> Value {
        json!({"id":self.item,"type":"message","role":"assistant","status":if done { "completed" } else { "in_progress" },"content":if done { vec![self.part(true)] } else { vec![] }})
    }
    fn response(&self, status: &str) -> Value {
        json!({"id":self.id,"object":"response","created_at":self.created_at,"status":status,"model":self.model,"output":if status == "completed" { self.items() } else { vec![] },"error":null,"incomplete_details":null,"usage":null,"parallel_tool_calls":false,"tools":self.tools,"tool_choice":self.choice})
    }
    fn items(&self) -> Vec<Value> {
        let mut items = vec![self.item(true)];
        items.extend(self.calls.clone());
        items
    }
    pub fn start(&mut self) -> String {
        let mut out = self.frame(
            "response.created",
            json!({"response":self.response("in_progress")}),
        );
        out += &self.frame(
            "response.in_progress",
            json!({"response":self.response("in_progress")}),
        );
        out += &self.frame(
            "response.output_item.added",
            json!({"output_index":0,"item":self.item(false)}),
        );
        out += &self.frame(
            "response.content_part.added",
            json!({"item_id":self.item,"output_index":0,"content_index":0,"part":self.part(false)}),
        );
        out
    }
    pub fn delta(&mut self, text: String) -> Result<String, &'static str> {
        let text = match &mut self.parser {
            Some(parser) => parser.push(&text)?,
            None => text,
        };
        if text.is_empty() {
            return Ok(String::new());
        }
        self.text.push_str(&text);
        Ok(self.frame("response.output_text.delta", json!({"item_id":self.item,"output_index":0,"content_index":0,"delta":text,"logprobs":[]})))
    }
    pub fn finish(&mut self) -> Result<String, &'static str> {
        if let Some(parser) = self.parser.take() {
            self.calls = parser.finish()?;
            for call in &mut self.calls {
                call["id"] = json!(format!("tool_{}", uuid::Uuid::new_v4().simple()));
                call["status"] = json!("completed");
            }
        }
        let mut out = self.frame("response.output_text.done", json!({"item_id":self.item,"output_index":0,"content_index":0,"text":self.text,"logprobs":[]}));
        out += &self.frame(
            "response.content_part.done",
            json!({"item_id":self.item,"output_index":0,"content_index":0,"part":self.part(true)}),
        );
        out += &self.frame(
            "response.output_item.done",
            json!({"output_index":0,"item":self.item(true)}),
        );
        for (index, call) in self.calls.clone().into_iter().enumerate() {
            let index = index + 1;
            let (field, event) = if call["type"] == "function_call" {
                ("arguments", "response.function_call_arguments")
            } else {
                ("input", "response.custom_tool_call_input")
            };
            let mut added = call.clone();
            added[field] = json!("");
            added["status"] = json!("in_progress");
            out += &self.frame(
                "response.output_item.added",
                json!({"output_index":index,"item":added}),
            );
            out += &self.frame(
                &format!("{event}.delta"),
                json!({"item_id":call["id"],"output_index":index,"delta":call[field]}),
            );
            let mut done = json!({"item_id":call["id"],"output_index":index});
            done[field] = call[field].clone();
            out += &self.frame(&format!("{event}.done"), done);
            out += &self.frame(
                "response.output_item.done",
                json!({"output_index":index,"item":call}),
            );
        }
        out += &self.frame(
            "response.completed",
            json!({"response":self.response("completed")}),
        );
        Ok(out)
    }
    pub fn fail(&mut self, code: &str) -> String {
        let mut response = self.response("failed");
        response["error"] = json!({"code":code,"message":format!("ChatGPT browser failed ({code}); request was not replayed")});
        self.frame("response.failed", json!({"response":response}))
    }
}
