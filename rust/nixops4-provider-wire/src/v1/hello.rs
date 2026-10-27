use std::{collections::BTreeMap, num::NonZeroU64};

use serde_json::json;

use crate::schema::v1 as gen;
use crate::v1::fault::{error, UNSUPPORTED_PROTOCOL};

pub const UNKNOWN_CAPABILITY_CODE: &str = "protocol.unknownCapability";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    Update,
    State,
    Progress,
    Cancel,
    Describe,
}

impl Capability {
    pub const ALL: [Capability; 5] = [
        Capability::Update,
        Capability::State,
        Capability::Progress,
        Capability::Cancel,
        Capability::Describe,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Capability::Update => "update",
            Capability::State => "state",
            Capability::Progress => "progress",
            Capability::Cancel => "cancel",
            Capability::Describe => "describe",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|capability| capability.name() == name)
    }

    pub fn accepted_versions(self) -> &'static [u8] {
        match self {
            Capability::State => &[1, 2],
            _ => &[1],
        }
    }
}

#[derive(Debug)]
pub struct Negotiated {
    capabilities: BTreeMap<Capability, NonZeroU64>,
    pub warnings: Vec<gen::Warning>,
}

impl Negotiated {
    pub fn version(&self, capability: Capability) -> Option<NonZeroU64> {
        self.capabilities.get(&capability).copied()
    }

    pub fn has(&self, capability: Capability) -> bool {
        self.version(capability).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Capability, NonZeroU64)> + '_ {
        self.capabilities.iter().map(|(c, v)| (*c, *v))
    }
}

#[derive(Debug)]
pub enum HandshakeFault {
    ProtocolNotOffered {
        got: String,
    },
    CapabilityNotOffered {
        capability: Capability,
        got: NonZeroU64,
    },
}

impl std::fmt::Display for HandshakeFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandshakeFault::ProtocolNotOffered { got } => {
                write!(f, "provider answered protocol {got}, which was not offered")
            }
            HandshakeFault::CapabilityNotOffered { capability, got } => write!(
                f,
                "provider answered {}@{}, which was not offered",
                capability.name(),
                got
            ),
        }
    }
}

impl std::error::Error for HandshakeFault {}

pub fn check_hello_answer(
    offered: &[gen::ProtocolId],
    offer: &gen::Offer,
    answer: &gen::HelloDone,
) -> Result<Negotiated, HandshakeFault> {
    if !offered.contains(&answer.protocol) {
        return Err(HandshakeFault::ProtocolNotOffered {
            got: answer.protocol.to_string(),
        });
    }
    let mut negotiated = Negotiated {
        capabilities: BTreeMap::new(),
        warnings: Vec::new(),
    };
    for (name, got) in &answer.capabilities.0 {
        let Some(capability) = Capability::parse(name) else {
            negotiated.warnings.push(gen::Warning {
                code: UNKNOWN_CAPABILITY_CODE.to_string(),
                message: format!("ignored unknown capability {name}"),
            });
            continue;
        };
        let offered_versions = offer.0.get(name);
        if !offered_versions.is_some_and(|versions| versions.contains(got)) {
            return Err(HandshakeFault::CapabilityNotOffered {
                capability,
                got: *got,
            });
        }
        negotiated.capabilities.insert(capability, *got);
    }
    Ok(negotiated)
}

pub fn unsupported_protocol_error(supported: &[gen::ProtocolId]) -> gen::Error {
    let mut fault = error(UNSUPPORTED_PROTOCOL.into(), "unsupported protocol");
    fault.data = Some(json!({
        "supported": supported
            .iter()
            .map(|protocol| protocol.to_string())
            .collect::<Vec<String>>(),
    }));
    fault
}

impl gen::HelloDone {
    pub fn max_concurrency(&self) -> NonZeroU64 {
        self.max_concurrency.unwrap_or(NonZeroU64::MIN)
    }

    pub fn shared_session(&self) -> bool {
        self.shared_session.unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offered() -> Vec<gen::ProtocolId> {
        vec!["resource-provider/1".to_string().try_into().unwrap()]
    }

    fn offer_of(pairs: &[(&str, &[u64])]) -> gen::Offer {
        gen::Offer(
            pairs
                .iter()
                .map(|(name, versions)| {
                    (
                        name.to_string(),
                        versions
                            .iter()
                            .map(|v| NonZeroU64::new(*v).unwrap())
                            .collect(),
                    )
                })
                .collect(),
        )
    }

    fn answer_of(pairs: &[(&str, u64)]) -> gen::Capabilities {
        gen::Capabilities(
            pairs
                .iter()
                .map(|(name, version)| (name.to_string(), NonZeroU64::new(*version).unwrap()))
                .collect(),
        )
    }

    fn hello_done(capabilities: gen::Capabilities) -> gen::HelloDone {
        gen::HelloDone {
            protocol: "resource-provider/1".to_string().try_into().unwrap(),
            provider: gen::ProviderInfo {
                name: "nixops4-resources-local".to_string(),
                version: "0.2.0".to_string(),
            },
            capabilities,
            max_concurrency: None,
            shared_session: None,
        }
    }

    #[test]
    fn negotiates_offered_capabilities() {
        let offer = offer_of(&[("update", &[1]), ("state", &[1, 2])]);
        let done = hello_done(answer_of(&[("update", 1), ("state", 2)]));
        let negotiated = check_hello_answer(&offered(), &offer, &done).unwrap();
        assert_eq!(negotiated.version(Capability::Update), NonZeroU64::new(1));
        assert_eq!(negotiated.version(Capability::State), NonZeroU64::new(2));
        assert!(!negotiated.has(Capability::Progress));
        assert_eq!(done.max_concurrency(), NonZeroU64::MIN);
        assert!(!done.shared_session());
    }

    #[test]
    fn rejects_protocol_that_was_not_offered() {
        let offer = offer_of(&[]);
        let mut done = hello_done(answer_of(&[]));
        done.protocol = "resource-provider/2".to_string().try_into().unwrap();
        match check_hello_answer(&offered(), &offer, &done) {
            Err(HandshakeFault::ProtocolNotOffered { got }) => {
                assert_eq!(got, "resource-provider/2")
            }
            other => panic!("expected ProtocolNotOffered, got {other:?}"),
        }
    }

    #[test]
    fn rejects_version_that_was_not_offered() {
        let offer = offer_of(&[("state", &[1])]);
        let done = hello_done(answer_of(&[("state", 2)]));
        match check_hello_answer(&offered(), &offer, &done) {
            Err(HandshakeFault::CapabilityNotOffered { capability, got }) => {
                assert_eq!(capability, Capability::State);
                assert_eq!(got.get(), 2);
            }
            other => panic!("expected CapabilityNotOffered, got {other:?}"),
        }
    }

    #[test]
    fn rejects_capability_missing_from_the_offer() {
        let offer = offer_of(&[]);
        let done = hello_done(answer_of(&[("update", 1)]));
        match check_hello_answer(&offered(), &offer, &done) {
            Err(HandshakeFault::CapabilityNotOffered { capability, got }) => {
                assert_eq!(capability, Capability::Update);
                assert_eq!(got.get(), 1);
            }
            other => panic!("expected CapabilityNotOffered, got {other:?}"),
        }
    }

    #[test]
    fn unknown_capabilities_are_warned_not_fatal() {
        let offer = offer_of(&[]);
        let done = hello_done(answer_of(&[("read", 1)]));
        let negotiated = check_hello_answer(&offered(), &offer, &done).unwrap();
        assert_eq!(negotiated.warnings.len(), 1);
        assert_eq!(negotiated.warnings[0].code, UNKNOWN_CAPABILITY_CODE);
    }
}
