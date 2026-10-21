use napi::{Error, Result};
use napi_derive::napi;

#[napi]
pub fn protocol_version() -> u32 {
    1
}

#[napi]
pub struct LoopDriver {
    inner: Option<pi_kernel::LoopDriver>,
}

#[napi]
impl LoopDriver {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Some(pi_kernel::LoopDriver::default()),
        }
    }

    #[napi]
    pub fn action(&self) -> Result<String> {
        self.inner
            .as_ref()
            .map(|driver| driver.action().as_str().to_owned())
            .ok_or_else(|| Error::from_reason("Loop driver is closed"))
    }

    #[napi]
    pub fn advance(&mut self, feedback: String) -> Result<String> {
        self.inner
            .as_mut()
            .ok_or_else(|| Error::from_reason("Loop driver is closed"))?
            .advance(&feedback)
            .map(|action| action.as_str().to_owned())
            .map_err(Error::from_reason)
    }

    #[napi]
    pub fn close(&mut self) {
        self.inner = None;
    }
}

impl Default for LoopDriver {
    fn default() -> Self {
        Self::new()
    }
}
