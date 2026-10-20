//! Actor and host information returned by composite registration, without another hydration call.

use crate::ActorInfo;
use crate::AgentHostAuthorization;

pub struct AuthenticatedActor {
    pub(crate) info: ActorInfo,
}

impl AuthenticatedActor {
    pub fn info(&self) -> &ActorInfo {
        &self.info
    }
}

pub struct AgentHost {
    pub(crate) id: String,
    pub(crate) authorization: AgentHostAuthorization,
}

impl AgentHost {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn authorization(&self) -> &AgentHostAuthorization {
        &self.authorization
    }
}
