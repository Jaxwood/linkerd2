use std::sync::Arc;
use tokio::sync::{AcquireError, OwnedSemaphorePermit, Semaphore};

/// Issues handles and waits for them to be released
pub struct Initialize {
    semaphore: Arc<Semaphore>,
    issued: u32,
}

/// Signals a component has been initialized
#[must_use]
pub struct Handle(OwnedSemaphorePermit);

impl Default for Initialize {
    fn default() -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(0)),
            issued: 0,
        }
    }
}

impl Initialize {
    /// Creates a new [`Handle`]
    pub fn add_handle(&mut self) -> Handle {
        let sem = self.semaphore.clone();
        sem.add_permits(1);
        let permit = sem
            .try_acquire_owned()
            .expect("semaphore must issue permit");
        self.issued += 1;
        Handle(permit)
    }

    /// Waits for all handles to be dropped.
    pub async fn initialized(self) -> Result<(), AcquireError> {
        let _ = self.semaphore.acquire_many(self.issued).await?;
        Ok(())
    }
}

impl Handle {
    /// Releases the handle
    pub async fn release(self) {
        self.0.release().await
    }
}
