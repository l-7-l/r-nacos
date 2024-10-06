use std::fmt::{Display, Formatter};
use std::{collections::HashMap, convert::TryFrom, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::common::model::{TokenSession, UserSession};

#[derive(Clone, prost::Message, Serialize, Deserialize)]
pub struct CacheItemDo {
    #[prost(uint32, tag = "1")]
    pub cache_type: u32,
    #[prost(bytes = "vec", tag = "2")]
    pub data: Vec<u8>,
    #[prost(int32, tag = "3")]
    pub timeout: i32,
}

impl CacheItemDo {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        prost::Message::encode(self, &mut v).unwrap_or_default();
        v
    }

    pub fn from_bytes(v: &[u8]) -> anyhow::Result<Self> {
        Ok(prost::Message::decode(v)?)
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum CacheType {
    String,
    Map,
    UserSession,
    ApiTokenSession, //open api
}

impl Default for CacheType {
    fn default() -> Self {
        Self::String
    }
}

impl CacheType {
    pub fn get_type_data(&self) -> u8 {
        match self {
            CacheType::String => 1,
            CacheType::Map => 2,
            CacheType::UserSession => 3,
            CacheType::ApiTokenSession => 4,
        }
    }

    pub fn from_data(v: u8) -> anyhow::Result<Self> {
        match v {
            1 => Ok(CacheType::String),
            2 => Ok(CacheType::Map),
            3 => Ok(CacheType::UserSession),
            4 => Ok(CacheType::ApiTokenSession),
            _ => Err(anyhow::anyhow!("unknown type from {}", &v)),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Hash, Default)]
pub struct CacheKey {
    pub cache_type: CacheType,
    pub key: Arc<String>,
}

impl CacheKey {
    pub fn to_key_string(&self) -> String {
        format!("{}\x00{}", self.cache_type.get_type_data(), &self.key)
    }
}

impl Display for CacheKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\x00{}", self.cache_type.get_type_data(), &self.key)
    }
}

impl CacheKey {
    pub fn new(cache_type: CacheType, key: Arc<String>) -> Self {
        Self { cache_type, key }
    }

    pub fn from_db_key(db_key: Vec<u8>) -> anyhow::Result<Self> {
        let mut iter = db_key.split(|v| *v == 0);
        let t = if let Some(t) = iter.next() {
            String::from_utf8(t.to_owned())?.parse::<u8>()?
        } else {
            return Err(anyhow::anyhow!("db_key split type is error!"));
        };
        if let Some(key) = iter.next() {
            Self::from_bytes(key.to_owned(), t)
        } else {
            Err(anyhow::anyhow!("db_key split key is error!"))
        }
    }

    fn from_bytes(key: Vec<u8>, t: u8) -> anyhow::Result<Self> {
        let key = String::from_utf8(key)?;
        Self::from_string(key, t)
    }

    fn from_string(key: String, t: u8) -> anyhow::Result<Self> {
        let cache_type = CacheType::from_data(t)?;
        Ok(Self {
            cache_type,
            key: Arc::new(key),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CacheValue {
    String(Arc<String>),
    Map(Arc<HashMap<String, String>>),
    //后面UserSession换成定义好的对象
    UserSession(Arc<UserSession>),
    ApiTokenSession(Arc<TokenSession>),
}

impl Default for CacheValue {
    fn default() -> Self {
        Self::String(Default::default())
    }
}

impl CacheValue {
    pub fn get_cache_type(&self) -> CacheType {
        match self {
            CacheValue::String(_) => CacheType::String,
            CacheValue::Map(_) => CacheType::Map,
            CacheValue::UserSession(_) => CacheType::UserSession,
            CacheValue::ApiTokenSession(_) => CacheType::ApiTokenSession,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            CacheValue::String(v) => v.as_bytes().to_owned(),
            CacheValue::Map(m) => serde_json::to_vec(m).unwrap_or_default(),
            CacheValue::UserSession(v) => serde_json::to_vec(v).unwrap_or_default(),
            CacheValue::ApiTokenSession(v) => serde_json::to_vec(v).unwrap_or_default(),
        }
    }

    pub fn from_bytes(data: Vec<u8>, cache_type: CacheType) -> anyhow::Result<Self> {
        match cache_type {
            CacheType::String => Ok(CacheValue::String(Arc::new(String::from_utf8(data)?))),
            CacheType::Map => Ok(CacheValue::Map(Arc::new(serde_json::from_slice(&data)?))),
            CacheType::UserSession => Ok(CacheValue::UserSession(Arc::new(
                serde_json::from_slice(&data)?,
            ))),
            CacheType::ApiTokenSession => {
                Ok(CacheValue::ApiTokenSession(serde_json::from_slice(&data)?))
            }
        }
    }
}

impl TryFrom<CacheItemDo> for CacheValue {
    type Error = anyhow::Error;
    fn try_from(value: CacheItemDo) -> Result<Self, Self::Error> {
        let cache_type = CacheType::from_data(value.cache_type as u8)?;
        Self::from_bytes(value.data, cache_type)
    }
}

impl From<CacheValue> for CacheItemDo {
    fn from(value: CacheValue) -> Self {
        Self {
            data: value.to_bytes(),
            timeout: 0,
            cache_type: value.get_cache_type().get_type_data() as u32,
        }
    }
}
