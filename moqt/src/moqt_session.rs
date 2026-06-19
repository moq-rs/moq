use crate::moqt_const::{
    kDefaultInitialMaxRequestId, kDefaultMaxAuthTokenCacheSize, kDefaultMoqtVersion,
};
use crate::moqt_key_value_pair::AuthToken;
use crate::moqt_types::Perspective;

pub struct MoqtSessionParameters {
    version: String,
    deliver_partial_objects: bool,
    perspective: Perspective,
    using_webtrans: bool,
    path: String,
    max_request_id: u64,
    max_auth_token_cache_size: u64,
    support_object_acks: bool,

    authorization_token: Vec<AuthToken>,
    authority: String,
    moqt_implementation: String,
}

impl Default for MoqtSessionParameters {
    fn default() -> Self {
        Self {
            version: kDefaultMoqtVersion.to_string(),
            deliver_partial_objects: false,
            perspective: Perspective::IS_SERVER,
            using_webtrans: true,
            path: "".to_string(),
            max_request_id: kDefaultInitialMaxRequestId,
            max_auth_token_cache_size: kDefaultMaxAuthTokenCacheSize,
            support_object_acks: false,
            authorization_token: vec![],
            authority: "".to_string(),
            moqt_implementation: "".to_string(),
        }
    }
}
