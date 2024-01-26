use std::{
    borrow::Borrow,
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

/// The one state that a backend instance needs during its lifetime
#[derive(Debug)]
pub struct SessionLayer<SessionKey, SessionHandle> {
    /// Mapping from a key to the session
    key_to_session: RwLock<HashMap<SessionKey, (SessionHandle, Mutex<Instant>)>>,
    /// Used to clean up the map and avoid memory leak
    timeout: Duration,
}
impl<SessionKey, SessionHandle> SessionLayer<SessionKey, SessionHandle>
where
    SessionKey: Sync + Send + 'static,
    SessionHandle: Sync + Send + 'static,
{
    pub fn new(timeout: Duration) -> Arc<Self> {
        let this = Arc::new(Self {
            key_to_session: RwLock::new(HashMap::new()),
            timeout,
        });

        // Clean the map routinely
        let weak_this = Arc::downgrade(&this);
        let check = timeout.div_f64(2.0);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(check).await;
                let this = weak_this.upgrade()?;
                this.remove_outdated();
            }
            #[allow(unreachable_code)]
            Some(())
        });

        this
    }

    fn remove_outdated(&self) {
        let now = Instant::now();
        let mut key_to_session = self.key_to_session.write().unwrap();
        key_to_session.retain(|_k, (_, time)| {
            let time = time.get_mut().unwrap();
            now - *time < self.timeout
        });
    }
}
impl<SK, SH> SessionLayer<SK, SH>
where
    SK: SessionKey,
    SH: SessionHandle,
{
    /// Clone out the session handle
    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<SH>
    where
        SK: Borrow<Q>,
        Q: Eq + std::hash::Hash,
    {
        let key_to_session = self.key_to_session.read().unwrap();
        let (session, time) = key_to_session.get(key)?;
        let mut time = time.lock().unwrap();
        *time = Instant::now();
        Some(session.clone())
    }

    pub fn insert(&self, key: SK, session: SH) -> Result<(), SessionCollision<SH>> {
        let mut key_to_session = self.key_to_session.write().unwrap();
        if key_to_session.get(&key).is_some() {
            return Err(SessionCollision(session));
        }
        key_to_session.insert(key, (session, Mutex::new(Instant::now())));
        Ok(())
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("session collision: {0}")]
pub struct SessionCollision<SessionHandle: std::fmt::Debug>(pub SessionHandle);

pub trait SessionHandle: std::fmt::Debug + Clone {}

pub trait SessionKey: std::fmt::Debug + std::hash::Hash + Eq + Clone {}
impl SessionKey for String {}
impl SessionKey for Vec<u8> {}
impl SessionKey for u128 {}
impl SessionKey for u64 {}
impl SessionKey for u32 {}
impl SessionKey for u8 {}
