use std::collections::BTreeMap;

use serde_json::json;

use crate::schema::v1 as gen;
use crate::v1::envelope::{parse_message, Answer, MessageKind, ResponseFault};
use crate::v1::fault::FaultClass;
use crate::v1::id::{IdSource, TypedId};
use crate::v1::method::{Create, Method, MethodName};
use crate::v1::outcome::OperationResult;

fn typed_id<M: Method>(id: u64) -> TypedId<M> {
    let mut source = IdSource::new();
    let mut typed = source.next_typed::<M>().unwrap();
    while typed.id().as_u64() < id {
        typed = source.next_typed::<M>().unwrap();
    }
    typed
}

fn hello_done() -> gen::HelloDone {
    serde_json::from_value(json!({
        "protocol": "resource-provider/1",
        "provider": {"name": "nixops4-resources-local", "version": "0.2.0"},
        "capabilities": {"update": 1, "state": 1}
    }))
    .unwrap()
}

#[test]
fn hello_negotiates_from_the_example_response() {
    let line = include_str!("../../examples/v1/HelloResponse.json");
    let MessageKind::Response(response) = parse_message(line).unwrap() else {
        panic!("expected a response");
    };
    let pending = IdSource::new()
        .next_typed::<crate::v1::method::Hello>()
        .unwrap();
    let Answer::Result(result) = response
        .answer::<crate::v1::method::Hello>(pending)
        .unwrap()
    else {
        panic!("expected a result");
    };
    let crate::v1::outcome::Outcome::Done(done) = result.kind else {
        panic!("expected done");
    };
    let offered = vec!["resource-provider/1".to_string().try_into().unwrap()];
    let offer = gen::Offer(BTreeMap::from([
        ("update".to_string(), vec![1.try_into().unwrap()]),
        (
            "state".to_string(),
            vec![1.try_into().unwrap(), 2.try_into().unwrap()],
        ),
    ]));
    let negotiated = crate::v1::check_hello_answer(&offered, &offer, &done).unwrap();
    assert_eq!(done.max_concurrency().get(), 4);
    assert!(done.shared_session() == false);
    assert!(negotiated.has(crate::v1::Capability::Update));
    assert!(negotiated.has(crate::v1::Capability::State));
    assert!(!negotiated.has(crate::v1::Capability::Describe));
    assert_eq!(hello_done().provider.name, "nixops4-resources-local");
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
