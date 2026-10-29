use serde_json::json;

use crate::schema::v1 as gen;
use crate::v1::envelope::{parse_message, Answer, MessageKind, ResponseFault};
use crate::v1::fault::FaultClass;
use crate::v1::method::{Create, Hello, MethodName};
use crate::v1::outcome::OperationResult;
use crate::v1::test_support::{offer_of, offered_protocols, typed_id};

#[test]
fn hello_negotiates_from_the_example_response() {
    let line = include_str!("../../examples/v1/HelloResponse.json");
    let MessageKind::Response(response) = parse_message(line).unwrap() else {
        panic!("expected a response");
    };
    let pending = typed_id::<Hello>(0);
    let Answer::Result(result) = response.answer::<Hello>(pending).unwrap() else {
        panic!("expected a result");
    };
    let crate::v1::outcome::Outcome::Done(done) = result.kind else {
        panic!("expected done");
    };
    let offer = offer_of(&[("update", &[1]), ("state", &[1, 2])]);
    let negotiated = crate::v1::check_hello_answer(&offered_protocols(), &offer, &done).unwrap();
    assert_eq!(done.max_concurrency().get(), 4);
    assert!(!done.shared_session());
    assert!(negotiated.has(crate::v1::Capability::Update));
    assert!(negotiated.has(crate::v1::Capability::State));
    assert!(!negotiated.has(crate::v1::Capability::Describe));
}

#[test]
fn create_request_decodes_by_method_on_the_server_side() {
    let line = include_str!("../../examples/v1/CreateRequest.json");
    let MessageKind::Request(request) = parse_message(line).unwrap() else {
        panic!("expected a request");
    };
    assert_eq!(request.method_name(), Some(MethodName::Create));
    let crate::v1::AnyRequest::Create(params) = request.decode_params().unwrap() else {
        panic!("expected create params");
    };
    assert_eq!(params.type_.0, "file");
    assert!(params.is_stateful);
    assert_eq!(params.label.as_deref(), Some("web.motd"));
}

#[test]
fn unknown_methods_are_distinguishable_from_malformed_params() {
    let request: gen::Request = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "configure",
        "params": {},
    }))
    .unwrap();
    assert_eq!(request.method_name(), None);
    assert_eq!(
        request.decode_params().unwrap_err().to_string(),
        "unknown method: configure"
    );

    let request: gen::Request = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "create",
        "params": {"type": "file"},
    }))
    .unwrap();
    assert!(matches!(
        request.decode_params().unwrap_err(),
        crate::v1::ParamsError::MalformedParams(_)
    ));

    let request: gen::Request = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "create",
        "params": ["file", {}],
    }))
    .unwrap();
    assert!(matches!(
        request.decode_params().unwrap_err(),
        crate::v1::ParamsError::MalformedParams(_)
    ));
}

#[test]
fn v0_provider_lines_are_not_json_rpc() {
    for line in [
        r#"{"errorResponse":{"message":"unknown request type"}}"#,
        r#"{"createResourceResponse":{"outputProperties":{}}}"#,
    ] {
        match parse_message(line) {
            Err(crate::v1::MessageParseError::NotJsonRpc) => {}
            other => panic!("expected NotJsonRpc for {line}, got {other:?}"),
        }
    }
}

#[test]
fn torn_empty_and_garbage_padded_lines_are_not_json() {
    for line in [
        r#"{"jsonrpc":"2.0","id":1,"method":"#,
        r#"{"jsonrpc":"2.0","id":1} trailing output"#,
        "",
    ] {
        match parse_message(line) {
            Err(crate::v1::MessageParseError::NotJson(_)) => {}
            other => panic!("expected NotJson for {line:?}, got {other:?}"),
        }
    }
}

#[test]
fn wrong_jsonrpc_versions_are_malformed() {
    let line = r#"{"jsonrpc":"1.1","id":1,"method":"hello","params":{}}"#;
    assert!(matches!(
        parse_message(line).unwrap_err(),
        crate::v1::MessageParseError::MalformedMessage(_)
    ));
}

#[test]
fn null_request_ids_are_malformed() {
    let line = r#"{"jsonrpc":"2.0","id":null,"method":"create","params":{}}"#;
    assert!(matches!(
        parse_message(line).unwrap_err(),
        crate::v1::MessageParseError::MalformedMessage(_)
    ));
}

#[test]
fn errors_missing_their_message_are_malformed() {
    let line = r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32601}}"#;
    assert!(matches!(
        parse_message(line).unwrap_err(),
        crate::v1::MessageParseError::MalformedMessage(_)
    ));
}

#[test]
fn null_error_ids_are_id_mismatches() {
    let line = r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}"#;
    let MessageKind::Response(response) = parse_message(line).unwrap() else {
        panic!("expected a response");
    };
    match response.answer::<Create>(typed_id::<Create>(0)) {
        Err(ResponseFault::IdMismatch { got }) => assert_eq!(got, None),
        other => panic!("expected IdMismatch, got {other:?}"),
    }
}

#[test]
fn negative_response_ids_are_id_mismatches() {
    let line = r#"{"jsonrpc":"2.0","id":-3,"result":{"outcome":"done","outputProperties":{}}}"#;
    let MessageKind::Response(response) = parse_message(line).unwrap() else {
        panic!("expected a response");
    };
    match response.answer::<Create>(typed_id::<Create>(0)) {
        Err(ResponseFault::IdMismatch { got }) => assert_eq!(got, Some(-3)),
        other => panic!("expected IdMismatch, got {other:?}"),
    }
}

#[test]
fn responses_with_both_result_and_error_are_faults() {
    let line = r#"{"jsonrpc":"2.0","id":7,"result":{"outcome":"done","outputProperties":{}},"error":{"code":-32001,"message":"unsupported protocol"}}"#;
    let MessageKind::Response(response) = parse_message(line).unwrap() else {
        panic!("expected a response");
    };
    match response.answer::<Create>(typed_id::<Create>(7)) {
        Err(ResponseFault::BothResultAndError) => {}
        other => panic!("expected BothResultAndError, got {other:?}"),
    }
}

#[test]
fn done_results_missing_their_payload_are_malformed() {
    let line = r#"{"jsonrpc":"2.0","id":7,"result":{"outcome":"done"}}"#;
    let MessageKind::Response(response) = parse_message(line).unwrap() else {
        panic!("expected a response");
    };
    match response.answer::<Create>(typed_id::<Create>(7)) {
        Err(ResponseFault::MalformedResult(_)) => {}
        other => panic!("expected MalformedResult, got {other:?}"),
    }
}

#[test]
fn non_object_results_are_malformed() {
    for result in [r#"[{"outputProperties":{}}]"#, r#""ok""#] {
        let line = format!(r#"{{"jsonrpc":"2.0","id":7,"result":{result}}}"#);
        let MessageKind::Response(response) = parse_message(&line).unwrap() else {
            panic!("expected a response");
        };
        match response.answer::<Create>(typed_id::<Create>(7)) {
            Err(ResponseFault::MalformedResult(_)) => {}
            other => panic!("expected MalformedResult for {line}, got {other:?}"),
        }
    }
}

#[test]
fn unknown_notification_methods_are_tolerated() {
    let line = r#"{"jsonrpc":"2.0","method":"trace","params":{"depth":2}}"#;
    let MessageKind::Notification(notification) = parse_message(line).unwrap() else {
        panic!("expected a notification");
    };
    assert_eq!(notification.method, "trace");
}

#[test]
fn create_done_decodes_by_the_pending_requests_method() {
    let line = include_str!("../../examples/v1/CreateDone.json");
    let pending = typed_id::<Create>(7);
    let MessageKind::Response(response) = parse_message(line).unwrap() else {
        panic!("expected a response");
    };
    let Answer::Result(result) = response.answer::<Create>(pending).unwrap() else {
        panic!("expected a result");
    };
    let crate::v1::outcome::Outcome::Done(done) = result.kind else {
        panic!("expected done");
    };
    assert!(done.output_properties.0.is_empty());
}

#[test]
fn id_mismatch_is_reported_before_anything_is_decoded() {
    let line = include_str!("../../examples/v1/CreateDone.json");
    let pending = typed_id::<Create>(8);
    let mut response: gen::Response = serde_json::from_str(line).unwrap();
    response.id = gen::ResponseId(Some(pending.id().as_u64() as i64 + 1));
    match response.answer::<Create>(pending) {
        Err(ResponseFault::IdMismatch { got }) => assert_eq!(got, Some(9)),
        other => panic!("expected IdMismatch, got {other:?}"),
    }
}

#[test]
fn fault_responses_keep_their_code_and_class() {
    let line = include_str!("../../examples/v1/ErrorResponse.json");
    let pending = typed_id::<Create>(7);
    let MessageKind::Response(response) = parse_message(line).unwrap() else {
        panic!("expected a response");
    };
    let Answer::Fault(fault) = response.answer::<Create>(pending).unwrap() else {
        panic!("expected a fault");
    };
    assert_eq!(fault.code(), crate::v1::ErrorCode::UnsupportedProtocol);
    assert_eq!(fault.class(), FaultClass::TreatedAsIndeterminate);
}

#[test]
fn unknown_outcome_example_lands_as_indeterminate() {
    let line = include_str!("../../examples/v1/UnknownOutcome.json");
    let pending = typed_id::<Create>(7);
    let MessageKind::Response(response) = parse_message(line).unwrap() else {
        panic!("expected a response");
    };
    let Answer::Result(OperationResult {
        kind: crate::v1::outcome::Outcome::Indeterminate(indeterminate),
        ..
    }) = response.answer::<Create>(pending).unwrap()
    else {
        panic!("expected an indeterminate result");
    };
    assert_eq!(indeterminate.raw_outcome.as_deref(), Some("partial"));
    assert_eq!(indeterminate.code, "core.partial");
}

#[test]
fn responses_with_neither_result_nor_error_are_faults() {
    let pending = typed_id::<Create>(7);
    let response: gen::Response = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": pending.id().as_u64(),
    }))
    .unwrap();
    assert!(matches!(
        response.answer::<Create>(pending).unwrap_err(),
        ResponseFault::NoResultOrError
    ));
}

#[test]
fn batches_and_non_json_rpc_lines_are_session_faults() {
    assert!(matches!(
        parse_message("[1,2]").unwrap_err(),
        crate::v1::MessageParseError::Batch
    ));
    assert!(matches!(
        parse_message("\"hello\"").unwrap_err(),
        crate::v1::MessageParseError::NotJsonRpc
    ));
    assert!(matches!(
        parse_message("{}").unwrap_err(),
        crate::v1::MessageParseError::NotJsonRpc
    ));
}

#[test]
fn every_example_request_decodes_to_its_method() {
    macro_rules! assert_request {
        ($file:literal, $method:literal) => {{
            let line = include_str!(concat!("../../examples/v1/", $file));
            let MessageKind::Request(request) = parse_message(line).unwrap() else {
                panic!("expected a request in {}", $file);
            };
            assert_eq!(request.method, $method, "{}", $file);
            request.decode_params().unwrap();
        }};
    }
    assert_request!("HelloRequest.json", "hello");
    assert_request!("CreateRequest.json", "create");
    assert_request!("UpdateRequest.json", "update");
    assert_request!("StateReadRequest.json", "stateRead");
    assert_request!("StateEventRequest.json", "stateEvent");

    macro_rules! assert_notification {
        ($file:literal, $method:literal) => {{
            let line = include_str!(concat!("../../examples/v1/", $file));
            let MessageKind::Notification(notification) = parse_message(line).unwrap() else {
                panic!("expected a notification in {}", $file);
            };
            assert_eq!(notification.method, $method, "{}", $file);
        }};
    }
    assert_notification!("LogNotification.json", "log");
    assert_notification!("ProgressNotification.json", "progress");
    assert_notification!("CancelNotification.json", "cancel");

    macro_rules! assert_response {
        ($file:literal) => {{
            let line = include_str!(concat!("../../examples/v1/", $file));
            assert!(matches!(
                parse_message(line).unwrap(),
                MessageKind::Response(_)
            ));
        }};
    }
    assert_response!("HelloResponse.json");
    assert_response!("CreateDone.json");
    assert_response!("CreateRejected.json");
    assert_response!("CreateIndeterminate.json");
    assert_response!("ErrorResponse.json");
    assert_response!("UnknownOutcome.json");
}
