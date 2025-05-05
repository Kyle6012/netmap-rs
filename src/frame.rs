use std::ops::{Deref, DerefMut};

/// A zero-copy view of a packet in a Netmap ring
pub struct Frame<'a> {
    data: &'a [u8],
}

impl<'a> Frame<'a> {
    /// create a new frame from a byte slice
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// get the length of the frame
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// check if the frame is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// get the payload as a byte slice
    pub fn payload(&self) -> &[u8] {
        self.data
    }
}

impl<'a> Deref for Frame<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.data
    }
}
impl<'a> From<&'a [u8]> for Frame<'a> {
    fn from(data: &'a [u8]) -> Self {
        Self::new(data)
    }
}
