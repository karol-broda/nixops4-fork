# Run with: nix build .#checks.x86_64-linux.json-schema
{
  runCommand,
  jsonschema,
  json-schema-catalog-rs,
  jsonSchemaCatalogs,
  jq,
}:

runCommand "check-schemas"
  {
    nativeBuildInputs = [
      jsonschema
      json-schema-catalog-rs
      jsonSchemaCatalogs.json-patch-schemastore
      jq
    ];
  }
  ''
    (
      set -x;
      # Validate resource schema (before local schema resolution for JSON Patch references)
      jv http://json-schema.org/draft-04/schema# ${../rust/nixops4-provider-wire/resource-schema-v0.json}

      # Create temporary schema file with resolved references
      json-schema-catalog replace ${../rust/nixops4-provider-wire/resource-schema-v0.json} > resource-schema-v0.json.tmp

      # Validate again (not typically required, but should work)
      jv http://json-schema.org/draft-04/schema# resource-schema-v0.json.tmp

      jv resource-schema-v0.json.tmp#/definitions/CreateResourceRequest ${../rust/nixops4-provider-wire/examples/v0/CreateResourceRequest.json}
      jv resource-schema-v0.json.tmp#/definitions/CreateResourceResponse ${../rust/nixops4-provider-wire/examples/v0/CreateResourceResponse.json}

      # Validate resource schema v1
      jv http://json-schema.org/draft-07/schema# ${../rust/nixops4-provider-wire/resource-schema-v1.json}

      json-schema-catalog replace ${../rust/nixops4-provider-wire/resource-schema-v1.json} > resource-schema-v1.json.tmp

      jv http://json-schema.org/draft-07/schema# resource-schema-v1.json.tmp

      jv resource-schema-v1.json.tmp#/definitions/Request ${../rust/nixops4-provider-wire/examples/v1/HelloRequest.json}
      jv resource-schema-v1.json.tmp#/definitions/Request ${../rust/nixops4-provider-wire/examples/v1/CreateRequest.json}
      jv resource-schema-v1.json.tmp#/definitions/Request ${../rust/nixops4-provider-wire/examples/v1/UpdateRequest.json}
      jv resource-schema-v1.json.tmp#/definitions/Request ${../rust/nixops4-provider-wire/examples/v1/StateReadRequest.json}
      jv resource-schema-v1.json.tmp#/definitions/Request ${../rust/nixops4-provider-wire/examples/v1/StateEventRequest.json}
      jv resource-schema-v1.json.tmp#/definitions/Response ${../rust/nixops4-provider-wire/examples/v1/HelloResponse.json}
      jv resource-schema-v1.json.tmp#/definitions/Response ${../rust/nixops4-provider-wire/examples/v1/CreateDone.json}
      jv resource-schema-v1.json.tmp#/definitions/Response ${../rust/nixops4-provider-wire/examples/v1/CreateRejected.json}
      jv resource-schema-v1.json.tmp#/definitions/Response ${../rust/nixops4-provider-wire/examples/v1/CreateIndeterminate.json}
      jv resource-schema-v1.json.tmp#/definitions/Response ${../rust/nixops4-provider-wire/examples/v1/ErrorResponse.json}
      jv resource-schema-v1.json.tmp#/definitions/Response ${../rust/nixops4-provider-wire/examples/v1/UnknownOutcome.json}
      jv resource-schema-v1.json.tmp#/definitions/Notification ${../rust/nixops4-provider-wire/examples/v1/LogNotification.json}
      jv resource-schema-v1.json.tmp#/definitions/Notification ${../rust/nixops4-provider-wire/examples/v1/ProgressNotification.json}
      jv resource-schema-v1.json.tmp#/definitions/Notification ${../rust/nixops4-provider-wire/examples/v1/CancelNotification.json}

      # Validate state schema (needs local schema resolution for JSON Patch references)
      jv http://json-schema.org/draft-04/schema# ${../rust/nixops4/state-schema-v0.json}

      # Create temporary schema file with resolved references
      json-schema-catalog replace ${../rust/nixops4/state-schema-v0.json} > state-schema-v0.json.tmp

      jv state-schema-v0.json.tmp#/definitions/State ${../rust/nixops4/examples/state/v0/empty.json}
      jv state-schema-v0.json.tmp#/definitions/StateEvent ${../rust/nixops4/examples/state/v0/initial-event.json}

      # Validate documentation examples
      jq -s '.[0]' ${../doc/manual/src/state/snippets/state-file.json} | jv state-schema-v0.json.tmp#/definitions/StateEvent
      jq -s '.[1]' ${../doc/manual/src/state/snippets/state-file.json} | jv state-schema-v0.json.tmp#/definitions/StateEvent
      jv state-schema-v0.json.tmp#/definitions/State ${../doc/manual/src/state/snippets/resolved-state.json}
    )
    touch $out
  ''
