use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod request;
pub mod response;

// Used for shard to shard communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardDefaultModel {
    pub(crate) d: Option<Vec<u8>>,
    pub(crate) op: ShardOpCode,
    pub(crate) id: Uuid,
}

impl ShardDefaultModel {
    pub fn new<'a, T>(d: T, op: ShardOpCode) -> Self
    where
        T: Serialize + Deserialize<'a>,
    {
        // Serialize message with flexbuffers
        let mut flex_serializer = flexbuffers::FlexbufferSerializer::new();
        d.serialize(&mut flex_serializer).unwrap();

        Self {
            d: Some(flex_serializer.view().to_vec()),
            op,
            id: Uuid::new_v4(),
        }
    }

    pub fn data<'a, T>(&'a self) -> T
    where
        T: Serialize + Deserialize<'a>,
    {
        let d = self.d.as_ref().unwrap();
        let r = flexbuffers::Reader::get_root(d.as_slice()).unwrap();
        T::deserialize(r).unwrap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ShardOpCode {
    SendAsDefaultModelToClient(Uuid),
    GameEvent,
    Request,
    Response,
}

pub trait ShardOpCodeFetcher {
    fn op_code() -> ShardOpCode;
}
