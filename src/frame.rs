use std::borrow::Cow;
use std::ops::Deref;

/// A view of a packet, potentially zero-copy (for Netmap sys) or owned (for fallback).
pub struct Frame<'a> {
    data: Cow<'a, [u8]>,
}

impl<'a> Frame<'a> {
    /// Create a new frame from a borrowed byte slice (zero-copy).
    pub fn new_borrowed(data: &'a [u8]) -> Self {
        Self {
            data: Cow::Borrowed(data),
        }
    }

    /// Create a new frame from an owned vector of bytes (for fallback).
    pub fn new_owned(data: Vec<u8>) -> Self {
        Self {
            data: Cow::Owned(data),
        }
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
        self.data.as_ref()
    }
}

impl<'a> Deref for Frame<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.data.as_ref()
    }
}
impl<'a> From<&'a [u8]> for Frame<'a> {
    fn from(data: &'a [u8]) -> Self {
        Self::new_borrowed(data)
    }
}
