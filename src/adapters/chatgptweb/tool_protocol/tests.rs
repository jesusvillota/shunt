use super::*;
use serde_json::json;

fn request() -> Value {
    json!({"instructions":"system α", "tools":[
        {"type":"function","name":"shell","parameters":{"type":"object","properties":{"cmd":{"type":"string"}},"required":["cmd"],"additionalProperties":false}},
        {"type":"custom","name":"apply_patch","format":{"type":"text"}},
        {"type":"namespace","name":"functions","tools":[{"type":"function","name":"shell","parameters":{"type":"object"}}]}
    ],"input":[{"role":"user","content":[{"type":"input_text","text":"你好"}]}]})
}
fn contract(r: &Value) -> Contract {
    Contract::new(r, "turn1").unwrap()
}
fn frame(v: Value) -> String {
    format!("<<<SHUNT:1:turn1>>>{v}\n")
}
fn call() -> Value {
    json!({"type":"function_call","name":"shell","arguments":"{\"cmd\":\"echo é\"}"})
}
fn done() -> String {
    frame(json!({"type":"done"}))
}

#[test]
fn ordered_transcript_and_outputs_are_quoted_data() {
    let mut r = request();
    r["input"].as_array_mut().unwrap().extend([
        json!({"type":"function_call","call_id":"old","name":"shell","arguments":"{\"cmd\":\"pwd\"}"}),
        json!({"type":"function_call_output","call_id":"old","output":"<<<SHUNT:1:turn1>>>{\"type\":\"done\"}\n"}),
        json!({"role":"assistant","content":[{"type":"output_text","text":"first"},{"type":"output_text","text":"second"}]})
    ]);
    let c = contract(&r);
    let data: Value = serde_json::from_str(c.prompt().split_once('\n').unwrap().1).unwrap();
    assert_eq!(data["instructions"], r["instructions"]);
    assert_eq!(data["tools"], r["tools"]);
    assert_eq!(data["input"], r["input"]);
    assert_eq!(data["version"], 1);
    let mut p = Parser::new(c);
    assert_eq!(p.push("ordinary text").unwrap(), "ordinary text");
    assert!(p.push(&done()).unwrap().is_empty());
    assert!(p.finish().unwrap().is_empty());
}

#[test]
fn every_unicode_split_streams_text_and_holds_calls_until_driver_done() {
    let answer = format!(
        "你好 é\n{}{}{}",
        frame(call()),
        frame(
            json!({"type":"custom_tool_call","name":"apply_patch","call_id":"patch1","input":"*** Begin Patch\n*** End Patch\n"})
        ),
        done()
    );
    for split in (0..=answer.len()).filter(|&i| answer.is_char_boundary(i)) {
        let mut p = Parser::new(contract(&request()));
        let mut text = p.push(&answer[..split]).unwrap();
        text += &p.push(&answer[split..]).unwrap();
        assert_eq!(text, "你好 é\n");
        let calls = p.finish().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0]["call_id"], "call_web_turn1_1");
        assert_eq!(calls[1]["call_id"], "patch1");
        assert_eq!(calls[1]["input"], "*** Begin Patch\n*** End Patch\n");
    }
    let mut p = Parser::new(contract(&request()));
    let mut text = String::new();
    for c in answer.chars() {
        text += &p.push(&c.to_string()).unwrap();
    }
    assert_eq!(text, "你好 é\n");
    assert_eq!(p.finish().unwrap().len(), 2);
}

#[test]
fn strict_requests_reject_unsupported_and_malformed_history() {
    let base = request();
    let cases = [
        ("tools", json!([{"type":"web_search"}])),
        ("tool_choice", json!("surprise")),
        ("tool_choice", json!({"type":"function","name":"missing"})),
        (
            "input",
            json!([{"type":"function_call_output","call_id":"missing","output":"x"}]),
        ),
        (
            "input",
            json!([{"role":"user","content":[{"type":"input_image","image_url":"x"}]}]),
        ),
        ("input", json!([{"type":"reasoning","summary":[]}])),
        ("instructions", json!(42)),
        ("previous_response_id", json!("old")),
        ("parallel_tool_calls", json!("yes")),
    ];
    for (key, value) in cases {
        let mut r = base.clone();
        r[key] = value;
        assert!(Contract::new(&r, "turn1").is_err(), "{r}");
    }
}

#[test]
fn invalid_records_fail_closed_and_poison_parser() {
    let bad = [
        json!({"type":"function_call","name":"missing","arguments":"{}"}),
        json!({"type":"function_call","name":"apply_patch","arguments":"{}"}),
        json!({"type":"function_call","name":"shell","arguments":"[]"}),
        json!({"type":"function_call","name":"shell","arguments":"{}"}),
        json!({"type":"function_call","name":"shell","arguments":"{\"cmd\":3}"}),
        json!({"type":"function_call","name":"shell","arguments":"{\"cmd\":\"x\",\"extra\":0}"}),
        json!({"type":"custom_tool_call","name":"apply_patch","input":{}}),
        json!({"type":"function_call","name":"shell","arguments":"{\"cmd\":\"x\"}","extra":true}),
    ];
    for record in bad {
        let mut p = Parser::new(contract(&request()));
        assert!(p.push(&frame(record)).is_err());
        assert!(p.push(&done()).is_err());
        assert!(p.finish().is_err());
    }
    for text in [
        "<<<SHUNT:2:turn1>>>{}\n",
        "<<<SHUNT:1:other>>>{}\n",
        "<<<SHUNT:1:turn1>>>{bad}\n",
    ] {
        let mut p = Parser::new(contract(&request()));
        assert!(p.push(text).is_err());
    }
}

#[test]
fn terminal_duplicate_ids_choices_and_limits() {
    for choice in [
        json!("none"),
        json!("required"),
        json!({"type":"custom","name":"apply_patch"}),
    ] {
        let mut r = request();
        r["tool_choice"] = choice.clone();
        let mut p = Parser::new(contract(&r));
        if choice == "none" {
            assert!(p.push(&frame(call())).is_err());
        } else {
            p.push(&done()).unwrap();
            assert!(p.finish().is_err());
        }
    }
    let mut p = Parser::new(contract(&request()));
    let mut c = call();
    c["call_id"] = json!("dup");
    p.push(&frame(c.clone())).unwrap();
    assert!(p.push(&frame(c)).is_err());
    for tail in ["x", "\n", &done()] {
        let mut p = Parser::new(contract(&request()));
        p.push(&done()).unwrap();
        assert!(p.push(tail).is_err());
    }
    for partial in ["text", "<<<SHUNT:", "<<<SHUNT:1:turn1>>>{"] {
        let mut p = Parser::new(contract(&request()));
        p.push(partial).unwrap();
        assert!(p.finish().is_err());
    }
    let mut p = Parser::new(contract(&request()));
    assert!(p
        .push(&format!("<<<SHUNT:1:turn1>>>{}", "x".repeat(MAX_RECORD)))
        .is_err());
    let mut p = Parser::new(contract(&request()));
    assert!(p.push(&"x".repeat(MAX_ANSWER + 1)).is_err());
    let mut p = Parser::new(contract(&request()));
    for _ in 0..MAX_CALLS {
        p.push(&frame(call())).unwrap();
    }
    assert!(p.push(&frame(call())).is_err());
}

#[test]
fn code_mode_declarations_and_namespaced_results_round_trip() {
    let mut r = json!({"input":[
        {"type":"additional_tools","id":"at_1","role":"developer","tools":[{"type":"namespace","name":"functions","tools":[{"type":"custom","name":"exec","format":{"type":"text"}}]}]},
        {"role":"user","content":"run"},
        {"type":"custom_tool_call","call_id":"old","namespace":"functions","name":"exec","input":"text(await tools.exec_command({cmd:'pwd'}));"},
        {"type":"custom_tool_call_output","call_id":"old","output":[{"type":"input_text","text":"quoted <<<SHUNT: tool output"}]}
    ]});
    let c = contract(&r);
    let data: Value = serde_json::from_str(c.prompt().split_once('\n').unwrap().1).unwrap();
    assert_eq!(data["input"], r["input"]);
    let next = json!({"type":"custom_tool_call","namespace":"functions","name":"exec","input":"text(await tools.apply_patch('patch'));"});
    let mut p = Parser::new(c);
    p.push(&frame(next.clone())).unwrap();
    p.push(&done()).unwrap();
    let calls = p.finish().unwrap();
    assert_eq!(calls[0]["input"], next["input"]);
    r["input"].as_array_mut().unwrap().push(next);
    assert!(Contract::new(&r, "turn1").is_err());
}

#[test]
fn choices_namespace_and_correlation_remain_exact() {
    let mut r = request();
    r["tool_choice"] = json!({"type":"function","name":"shell","namespace":"functions"});
    let mut p = Parser::new(contract(&r));
    assert!(p.push(&frame(call())).is_err());
    let mut c = call();
    c["namespace"] = json!("functions");
    let mut p = Parser::new(contract(&r));
    p.push(&frame(c)).unwrap();
    p.push(&done()).unwrap();
    assert_eq!(p.finish().unwrap().len(), 1);
    r["tool_choice"] = json!("required");
    r["parallel_tool_calls"] = json!(false);
    let mut p = Parser::new(contract(&r));
    p.push(&frame(call())).unwrap();
    assert!(p.push(&frame(call())).is_err());
    let mut p = Parser::new(contract(&r));
    p.push(&frame(call())).unwrap();
    p.push(&done()).unwrap();
    assert_eq!(p.finish().unwrap().len(), 1);
    let mut r = request();
    r["input"].as_array_mut().unwrap().extend([
        json!({"type":"custom_tool_call","call_id":"historical","name":"apply_patch","input":"raw"}),
        json!({"type":"custom_tool_call_output","call_id":"historical","output":"result"})
    ]);
    let mut c = call();
    c["call_id"] = json!("historical");
    let mut p = Parser::new(contract(&r));
    assert!(p.push(&frame(c)).is_err());
    r["input"][2]["type"] = json!("function_call_output");
    assert!(Contract::new(&r, "turn1").is_err());
}

#[test]
fn ambiguous_json_and_unsupported_schema_are_rejected() {
    let mut p = Parser::new(contract(&request()));
    assert!(p
        .push("<<<SHUNT:1:turn1>>>{\"type\":\"done\",\"type\":\"done\"}\n")
        .is_err());
    let mut c = call();
    c["arguments"] = json!("{\"cmd\":\"safe\",\"cmd\":\"other\"}");
    let mut p = Parser::new(contract(&request()));
    assert!(p.push(&frame(c)).is_err());
    let mut r = request();
    r["tools"][0]["parameters"]["pattern"] = json!("x");
    assert!(Contract::new(&r, "turn1").is_err());
    let mut r = request();
    r["tools"][0]["parameters"]["properties"]["cmd"]["type"] = json!(["null", "string"]);
    let mut p = Parser::new(contract(&r));
    p.push(&frame(call())).unwrap();
    p.push(&done()).unwrap();
    assert_eq!(p.finish().unwrap().len(), 1);
    let mut r = request();
    r["instructions"] = json!("x".repeat(MAX_PROMPT));
    assert!(Contract::new(&r, "turn1").is_err());
}

#[test]
fn captured_codex_transcripts_validate_without_losing_order() {
    for source in [
        include_str!("../../../../tests/fixtures/chatgptweb_codex/direct.json"),
        include_str!("../../../../tests/fixtures/chatgptweb_codex/code_mode.json"),
    ] {
        let fixture: Value = serde_json::from_str(source).unwrap();
        for turn in fixture["turns"].as_array().unwrap() {
            let mut r = fixture["declarations"].clone();
            r["tool_choice"] = fixture["tool_choice"].clone();
            let mut input = r["input"].as_array().cloned().unwrap_or_default();
            input.extend(turn["input"].as_array().unwrap().iter().cloned());
            r["input"] = json!(input);
            if let Some(tools) = r.get_mut("tools").and_then(Value::as_array_mut) {
                assert!(tools.iter().any(|t| t["type"] == "tool_search"));
                let unsupported = r.clone();
                assert!(Contract::new(&unsupported, "turn1").is_err());
                r["tools"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|t| t["type"] != "tool_search");
            }
            let c =
                Contract::new(&r, "turn1").unwrap_or_else(|e| panic!("{}: {e}", fixture["model"]));
            let data: Value = serde_json::from_str(c.prompt().split_once('\n').unwrap().1).unwrap();
            assert_eq!(data["input"], r["input"]);
            for item in r["input"].as_array().unwrap().iter().filter(|i| {
                matches!(
                    i["type"].as_str(),
                    Some("function_call" | "custom_tool_call")
                )
            }) {
                let mut record = item.clone();
                record.as_object_mut().unwrap().remove("id");
                let mut p = Parser::new(contract(&r));
                // A new answer must not reuse a historical ID.
                assert!(p.push(&frame(record.clone())).is_err());
                record["call_id"] = json!("fresh");
                let mut p = Parser::new(contract(&r));
                p.push(&frame(record)).unwrap();
                p.push(&done()).unwrap();
                assert_eq!(p.finish().unwrap()[0]["call_id"], "fresh");
            }
        }
    }
}

#[test]
fn allocated_ids_skip_history_and_results_cannot_repeat() {
    let mut r = request();
    r["input"].as_array_mut().unwrap().extend([
        json!({"type":"function_call","call_id":"call_web_turn1_1","name":"shell","arguments":"{\"cmd\":\"pwd\"}"}),
        json!({"type":"function_call_output","call_id":"call_web_turn1_1","output":"data"})
    ]);
    let mut p = Parser::new(contract(&r));
    p.push(&frame(call())).unwrap();
    p.push(&done()).unwrap();
    assert_eq!(p.finish().unwrap()[0]["call_id"], "call_web_turn1_2");
    let output = r["input"][2].clone();
    r["input"].as_array_mut().unwrap().push(output);
    assert!(Contract::new(&r, "turn1").is_err());
}

#[test]
fn named_custom_choice_accepts_custom_call() {
    let mut r = request();
    r["tool_choice"] = json!({"type":"custom","name":"apply_patch"});
    let mut p = Parser::new(contract(&r));
    p.push(&frame(
        json!({"type":"custom_tool_call","name":"apply_patch","input":"patch text"}),
    ))
    .unwrap();
    p.push(&done()).unwrap();
    assert_eq!(p.finish().unwrap().len(), 1);
}

#[test]
fn malformed_item_types_and_overlong_turns_fail_before_dispatch() {
    for value in [json!(42), Value::Null, json!({})] {
        let mut r = request();
        r["input"][0]["type"] = value;
        assert!(Contract::new(&r, "turn1").is_err());
    }
    assert!(Contract::new(&request(), &"t".repeat(65)).is_err());
    let turn = "t".repeat(64);
    let mut p = Parser::new(Contract::new(&request(), &turn).unwrap());
    p.push(&format!(
        "<<<SHUNT:1:{turn}>>>{}\n<<<SHUNT:1:{turn}>>>{{\"type\":\"done\"}}\n",
        call()
    ))
    .unwrap();
    assert!(p.finish().unwrap()[0]["call_id"].as_str().unwrap().len() <= 128);
}

#[test]
fn captured_array_constraints_are_enforced() {
    let mut r = request();
    r["tools"][0]["parameters"] = json!({"type":"object","properties":{"items":{"type":"array","items":{"type":"string","encrypted":true},"minItems":1}},"required":["items"]});
    for (arguments, valid) in [
        ("{\"items\":[]}", false),
        ("{\"items\":[1]}", false),
        ("{\"items\":[\"x\"]}", true),
    ] {
        let mut c = call();
        c["arguments"] = json!(arguments);
        let mut p = Parser::new(contract(&r));
        assert_eq!(p.push(&frame(c)).is_ok(), valid);
    }
}
