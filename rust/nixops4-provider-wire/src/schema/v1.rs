mod generated {
    // This module only contains generated code.
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/generated/schema/resource/v1.rs"));
}
pub use generated::*;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn params_of(json: &str) -> Value {
        let request: Request = serde_json::from_str(json).unwrap();
        request.params.unwrap()
    }

    fn result_of(json: &str) -> Value {
        let response: Response = serde_json::from_str(json).unwrap();
        response.result.unwrap()
    }

    #[test]
    fn examples_v1_hello_request() {
        let json = include_str!("../../examples/v1/HelloRequest.json");
        let request: Request = serde_json::from_str(json).unwrap();
        assert_eq!(request.id, Id(0));
        assert_eq!(request.method, "hello");
        let params: HelloParams = serde_json::from_value(params_of(json)).unwrap();
        assert_eq!(
            params.protocols,
            vec!["resource-provider/1".to_string().try_into().unwrap()]
        );
        assert_eq!(params.engine.name, "nixops4");
        assert_eq!(
            params.offer.0.get("state").unwrap(),
            &vec![
                std::num::NonZeroU64::new(1).unwrap(),
                std::num::NonZeroU64::new(2).unwrap()
            ]
        );
    }

    #[test]
    fn examples_v1_hello_response() {
        let json = include_str!("../../examples/v1/HelloResponse.json");
        let response: Response = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, ResponseId(Some(0)));
        assert!(response.error.is_none());
        let result: HelloDone = serde_json::from_value(result_of(json)).unwrap();
        assert_eq!(
            result.protocol,
            "resource-provider/1".to_string().try_into().unwrap()
        );
        assert_eq!(result.provider.name, "nixops4-resources-local");
        assert_eq!(
            result.capabilities.0.get("update").unwrap(),
            &std::num::NonZeroU64::new(1).unwrap()
        );
        assert_eq!(result.max_concurrency, std::num::NonZeroU64::new(4));
        assert_eq!(result.shared_session, Some(false));
    }

    #[test]
    fn examples_v1_create_request() {
        let json = include_str!("../../examples/v1/CreateRequest.json");
        let request: Request = serde_json::from_str(json).unwrap();
        assert_eq!(request.id, Id(7));
        assert_eq!(request.method, "create");
        let params: CreateParams = serde_json::from_value(params_of(json)).unwrap();
        assert_eq!(params.type_, ResourceType("file".to_string()));
        assert!(params.is_stateful);
        assert_eq!(params.label.as_deref(), Some("web.motd"));
        assert_eq!(
            params.input_properties.0.get("name"),
            Some(&Value::String("motd.txt".to_string()))
        );
    }

    #[test]
    fn examples_v1_state_event_request() {
        let json = include_str!("../../examples/v1/StateEventRequest.json");
        let params: StateEventParams = serde_json::from_value(params_of(json)).unwrap();
        assert_eq!(params.event, "create");
        assert_eq!(params.nixops_version, "0.2.0");
        assert_eq!(params.patch.0.len(), 1);
        assert_eq!(
            params.resource.type_,
            ResourceType("state_file".to_string())
        );
    }

    #[test]
    fn examples_v1_notifications() {
        let log: Notification =
            serde_json::from_str(include_str!("../../examples/v1/LogNotification.json")).unwrap();
        assert_eq!(log.method, "log");
        let log_params: LogNotificationParams =
            serde_json::from_value(log.params.unwrap()).unwrap();
        assert_eq!(log_params.message, "waiting for volume");
        assert_eq!(log_params.request_id, Some(Id(7)));

        let progress: Notification =
            serde_json::from_str(include_str!("../../examples/v1/ProgressNotification.json"))
                .unwrap();
        let progress_params: ProgressNotificationParams =
            serde_json::from_value(progress.params.unwrap()).unwrap();
        assert_eq!(progress_params.fraction, Some(0.3));

        let cancel: Notification =
            serde_json::from_str(include_str!("../../examples/v1/CancelNotification.json"))
                .unwrap();
        let cancel_params: CancelNotificationParams =
            serde_json::from_value(cancel.params.unwrap()).unwrap();
        assert_eq!(cancel_params.request_id, Id(7));
    }

    #[test]
    fn unknown_fields_are_tolerated() {
        let mut params: CreateParams = serde_json::from_value(params_of(include_str!(
            "../../examples/v1/CreateRequest.json"
        )))
        .unwrap();
        params.label = None;
        let mut value = serde_json::to_value(&params).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("someFutureField".to_string(), Value::Bool(true));
        let params_again: CreateParams = serde_json::from_value(value).unwrap();
        assert_eq!(params_again.type_, params.type_);
    }

    #[test]
    fn generated_types_never_deny_unknown_fields() {
        let source =
            std::fs::read_to_string(concat!(env!("OUT_DIR"), "/generated/schema/resource/v1.rs"))
                .unwrap();
        assert!(
            !source.contains("deny_unknown_fields"),
            "v1 types must tolerate unknown protocol fields"
        );
    }
}
