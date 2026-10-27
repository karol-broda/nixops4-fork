use std::marker::PhantomData;

use crate::schema::v1 as gen;
use crate::v1::method::Method;

pub const MAX_ID: u64 = (1 << 53) - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(u64);

impl RequestId {
    pub const FIRST: RequestId = RequestId(0);

    pub fn try_from_u64(value: u64) -> Result<Self, IdRangeError> {
        if value <= MAX_ID {
            Ok(RequestId(value))
        } else {
            Err(IdRangeError {
                value: value as i128,
            })
        }
    }

    pub fn try_from_i64(value: i64) -> Result<Self, IdRangeError> {
        if let Ok(value) = u64::try_from(value) {
            Self::try_from_u64(value)
        } else {
            Err(IdRangeError {
                value: value as i128,
            })
        }
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }

    pub fn to_wire(self) -> gen::Id {
        gen::Id(self.0 as i64)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdRangeError {
    pub value: i128,
}

impl std::fmt::Display for IdRangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "request id {} is outside 0..={MAX_ID}", self.value)
    }
}

impl std::error::Error for IdRangeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdExhausted;

impl std::fmt::Display for IdExhausted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "request ids are exhausted for this session")
    }
}

impl std::error::Error for IdExhausted {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdSource {
    next: Option<RequestId>,
}

impl IdSource {
    pub fn new() -> Self {
        IdSource {
            next: Some(RequestId::FIRST),
        }
    }

    pub fn next(&mut self) -> Result<RequestId, IdExhausted> {
        let current = self.next.ok_or(IdExhausted)?;
        self.next = current
            .0
            .checked_add(1)
            .and_then(|value| RequestId::try_from_u64(value).ok());
        Ok(current)
    }

    pub fn next_typed<M: Method>(&mut self) -> Result<TypedId<M>, IdExhausted> {
        Ok(TypedId::new(self.next()?))
    }
}

impl Default for IdSource {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TypedId<M> {
    id: RequestId,
    _method: PhantomData<fn() -> M>,
}

impl<M> TypedId<M> {
    fn new(id: RequestId) -> Self {
        TypedId {
            id,
            _method: PhantomData,
        }
    }

    pub fn id(self) -> RequestId {
        self.id
    }
}

impl<M> Clone for TypedId<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M> Copy for TypedId<M> {}

impl<M> PartialEq for TypedId<M> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<M> Eq for TypedId<M> {}

impl<M> std::hash::Hash for TypedId<M> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<M: Method> std::fmt::Debug for TypedId<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TypedId({}, {})", self.id.0, M::NAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_reject_out_of_range_values() {
        assert_eq!(RequestId::try_from_u64(MAX_ID).unwrap().as_u64(), MAX_ID);
        assert_eq!(
            RequestId::try_from_u64(MAX_ID + 1).unwrap_err().value,
            (MAX_ID + 1) as i128
        );
        assert_eq!(RequestId::try_from_i64(-1).unwrap_err().value, -1);
    }

    #[test]
    fn id_source_is_strictly_monotonic() {
        let mut source = IdSource::new();
        let first = source.next().unwrap();
        let second = source.next().unwrap();
        assert_eq!(first.as_u64(), 0);
        assert_eq!(second.as_u64(), 1);
    }

    #[test]
    fn id_source_exhausts_instead_of_wrapping() {
        let mut source = IdSource::new();
        source.next = Some(RequestId::try_from_u64(MAX_ID).unwrap());
        assert_eq!(source.next().unwrap().as_u64(), MAX_ID);
        assert_eq!(source.next().unwrap_err(), IdExhausted);
    }
}
