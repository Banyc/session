use std::{borrow::Borrow, sync::Arc, time::Duration};

use crate::session::{SessionHandle, SessionKey, SessionLayer};

use tokio::sync::{Mutex as TokioMutex, OwnedMutexGuard};

/// Store sessions that can be mutated asyncrounously
#[derive(Debug)]
pub struct MutSessionLayer<SessionKey, MutSession> {
    session: Arc<SessionLayer<SessionKey, Session<MutSession>>>,
}
impl<SessionKey, MutSession> MutSessionLayer<SessionKey, MutSession>
where
    SessionKey: Sync + Send + 'static,
    MutSession: Sync + Send + 'static,
{
    pub fn new(timeout: Duration) -> Self {
        Self {
            session: SessionLayer::new(timeout),
        }
    }
}
impl<SK, S: MutSession> MutSessionLayer<SK, S>
where
    SK: SessionKey + Sync + Send + 'static,
{
    pub async fn get_mut<Q: ?Sized>(&self, key: &Q) -> Option<OwnedMutexGuard<S>>
    where
        SK: Borrow<Q>,
        Q: Eq + std::hash::Hash,
    {
        let session = self.session.get(key)?;
        let mut_session = Arc::clone(&session.0).lock_owned().await;
        Some(mut_session)
    }

    pub fn insert(&self, key: SK, mut_session: S) -> Result<(), MutSessionCollision> {
        let session = Session(Arc::new(TokioMutex::new(mut_session)));
        self.session
            .insert(key, session)
            .map_err(|_| MutSessionCollision)
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("mut session collision")]
pub struct MutSessionCollision;

/// Satisfy any bounds that [`SessionLayer`] requires
#[derive(Debug)]
struct Session<MutSession>(Arc<TokioMutex<MutSession>>);
impl<MS: MutSession> SessionHandle for Session<MS> {}
impl<MutSession> Clone for Session<MutSession> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

pub trait MutSession: std::fmt::Debug + Sync + Send + 'static {}
