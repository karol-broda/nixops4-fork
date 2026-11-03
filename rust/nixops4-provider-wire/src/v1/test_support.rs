use std::num::NonZeroU64;

use crate::schema::v1 as gen;
use crate::v1::id::{IdSource, TypedId};
use crate::v1::method::Method;

pub fn typed_id<M: Method>(id: u64) -> TypedId<M> {
    let mut source = IdSource::new();
    let mut typed = source.next_typed::<M>().unwrap();
    while typed.id().as_u64() < id {
        typed = source.next_typed::<M>().unwrap();
    }
    typed
}

pub fn offered_protocols() -> Vec<gen::ProtocolId> {
    vec!["resource-provider/1".to_string().try_into().unwrap()]
}

pub fn offer_of(pairs: &[(&str, &[u64])]) -> gen::Offer {
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

pub fn capabilities_of(pairs: &[(&str, u64)]) -> gen::Capabilities {
    gen::Capabilities(
        pairs
            .iter()
            .map(|(name, version)| (name.to_string(), NonZeroU64::new(*version).unwrap()))
            .collect(),
    )
}

pub fn hello_done_with(capabilities: gen::Capabilities) -> gen::HelloDone {
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
