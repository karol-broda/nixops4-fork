use serde::{de::DeserializeOwned, Serialize};

use crate::schema::v1 as gen;

mod sealed {
    pub trait Sealed {}
}

pub trait Method: sealed::Sealed + Sized + 'static {
    const NAME: &'static str;
    type Params: Serialize + DeserializeOwned;
    type Done: Serialize + DeserializeOwned;
}

pub struct Hello;
pub struct Create;
pub struct Update;
pub struct StateRead;
pub struct StateEvent;

impl sealed::Sealed for Hello {}
impl sealed::Sealed for Create {}
impl sealed::Sealed for Update {}
impl sealed::Sealed for StateRead {}
impl sealed::Sealed for StateEvent {}

impl Method for Hello {
    const NAME: &'static str = "hello";
    type Params = gen::HelloParams;
    type Done = gen::HelloDone;
}

impl Method for Create {
    const NAME: &'static str = "create";
    type Params = gen::CreateParams;
    type Done = gen::CreateDone;
}

impl Method for Update {
    const NAME: &'static str = "update";
    type Params = gen::UpdateParams;
    type Done = gen::UpdateDone;
}

impl Method for StateRead {
    const NAME: &'static str = "stateRead";
    type Params = gen::StateReadParams;
    type Done = gen::StateReadDone;
}

impl Method for StateEvent {
    const NAME: &'static str = "stateEvent";
    type Params = gen::StateEventParams;
    type Done = gen::StateEventDone;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MethodName {
    Hello,
    Create,
    Update,
    StateRead,
    StateEvent,
}

impl MethodName {
    pub const ALL: [MethodName; 5] = [
        MethodName::Hello,
        MethodName::Create,
        MethodName::Update,
        MethodName::StateRead,
        MethodName::StateEvent,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            MethodName::Hello => Hello::NAME,
            MethodName::Create => Create::NAME,
            MethodName::Update => Update::NAME,
            MethodName::StateRead => StateRead::NAME,
            MethodName::StateEvent => StateEvent::NAME,
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|method| method.as_str() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_parse_back_and_forth() {
        for method in MethodName::ALL {
            assert_eq!(MethodName::parse(method.as_str()), Some(method));
        }
        assert_eq!(MethodName::parse("configure"), None);
        assert_eq!(MethodName::parse(""), None);
    }
}
