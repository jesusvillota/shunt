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
        return Err("chatgptweb_browser is text only; remove tools");
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
}
impl Output {
    pub fn new(id: String, model: String) -> Self {
        Self {
            item: format!("msg_{}", uuid::Uuid::new_v4().simple()),
            id,
            model,
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
        json!({"id":self.id,"object":"response","created_at":self.created_at,"status":status,"model":self.model,"output":if status == "completed" { vec![self.item(true)] } else { vec![] },"error":null,"incomplete_details":null,"usage":null,"parallel_tool_calls":false,"tools":[],"tool_choice":"none"})
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
    pub fn delta(&mut self, text: String) -> String {
        self.text.push_str(&text);
        self.frame("response.output_text.delta", json!({"item_id":self.item,"output_index":0,"content_index":0,"delta":text,"logprobs":[]}))
    }
    pub fn finish(&mut self) -> String {
        let mut out = self.frame("response.output_text.done", json!({"item_id":self.item,"output_index":0,"content_index":0,"text":self.text,"logprobs":[]}));
        out += &self.frame(
            "response.content_part.done",
            json!({"item_id":self.item,"output_index":0,"content_index":0,"part":self.part(true)}),
        );
        out += &self.frame(
            "response.output_item.done",
            json!({"output_index":0,"item":self.item(true)}),
        );
        out += &self.frame(
            "response.completed",
            json!({"response":self.response("completed")}),
        );
        out
    }
    pub fn fail(&mut self, code: &str) -> String {
        let mut response = self.response("failed");
        response["error"] = json!({"code":code,"message":format!("ChatGPT browser failed ({code}); request was not replayed")});
        self.frame("response.failed", json!({"response":response}))
    }
}
