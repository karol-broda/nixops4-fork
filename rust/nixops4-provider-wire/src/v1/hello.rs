use std::{collections::BTreeMap, num::NonZeroU64};

use crate::schema::v1 as gen;

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
}

#[derive(Debug, thiserror::Error)]
pub enum HandshakeFault {
    #[error("provider answered protocol {got}, which was not offered")]
    ProtocolNotOffered { got: String },
    #[error("provider answered {}@{got}, which was not offered", capability.name())]
    CapabilityNotOffered {
        capability: Capability,
        got: NonZeroU64,
    },
}

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
    use crate::v1::test_support::{capabilities_of, hello_done_with, offer_of, offered_protocols};

    #[test]
    fn negotiates_offered_capabilities() {
        let offer = offer_of(&[("update", &[1]), ("state", &[1, 2])]);
        let done = hello_done_with(capabilities_of(&[("update", 1), ("state", 2)]));
        let negotiated = check_hello_answer(&offered_protocols(), &offer, &done).unwrap();
        assert_eq!(negotiated.version(Capability::Update), NonZeroU64::new(1));
        assert_eq!(negotiated.version(Capability::State), NonZeroU64::new(2));
        assert!(!negotiated.has(Capability::Progress));
        assert_eq!(done.max_concurrency(), NonZeroU64::MIN);
        assert!(!done.shared_session());
    }

    #[test]
    fn rejects_protocol_that_was_not_offered() {
        let offer = offer_of(&[]);
        let mut done = hello_done_with(capabilities_of(&[]));
        done.protocol = "resource-provider/2".to_string().try_into().unwrap();
        match check_hello_answer(&offered_protocols(), &offer, &done) {
            Err(HandshakeFault::ProtocolNotOffered { got }) => {
                assert_eq!(got, "resource-provider/2")
            }
            other => panic!("expected ProtocolNotOffered, got {other:?}"),
        }
    }

    #[test]
    fn rejects_version_that_was_not_offered() {
        let offer = offer_of(&[("state", &[1])]);
        let done = hello_done_with(capabilities_of(&[("state", 2)]));
        match check_hello_answer(&offered_protocols(), &offer, &done) {
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
        let done = hello_done_with(capabilities_of(&[("update", 1)]));
        match check_hello_answer(&offered_protocols(), &offer, &done) {
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
        let done = hello_done_with(capabilities_of(&[("read", 1)]));
        let negotiated = check_hello_answer(&offered_protocols(), &offer, &done).unwrap();
        assert_eq!(negotiated.warnings.len(), 1);
        assert_eq!(negotiated.warnings[0].code, UNKNOWN_CAPABILITY_CODE);
    }
}
