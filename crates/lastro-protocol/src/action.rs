//! StationEvent action codes. Only the three hackathon operations are valid.

use crate::error::ProtocolError;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Origin = 1,
    Transfer = 2,
    Reidentify = 3,
}

impl TryFrom<u8> for Action {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Origin),
            2 => Ok(Self::Transfer),
            3 => Ok(Self::Reidentify),
            _ => Err(ProtocolError::InvalidAction),
        }
    }
}
