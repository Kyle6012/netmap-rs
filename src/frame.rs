use std::borrow::Cow;
use std::ops::Deref;

/// A view of a packet, potentially zero-copy (for Netmap sys) or owned (for fallback).
pub struct Frame<'a> {
    data: Cow<'a, [u8]>,
}

impl<'a> Frame<'a> {
    /// Create a new frame from a borrowed byte slice (zero-copy).
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data: Cow::Borrowed(data),
        }
    }

    /// Create a new frame from a borrowed byte slice (zero-copy).
    /// This is an alias for `new` for backward compatibility.
    pub fn new_borrowed(data: &'a [u8]) -> Self {
        Self::new(data)
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

impl Deref for Frame<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.data.as_ref()
    }
}

impl<'a> From<&'a [u8]> for Frame<'a> {
    fn from(data: &'a [u8]) -> Self {
        Self::new(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_borrows_zero_copy() {
        let data = b"hello netmap";
        let frame = Frame::new(data);
        assert_eq!(frame.len(), data.len());
        assert_eq!(frame.payload(), data);
        assert!(matches!(frame.data, Cow::Borrowed(_)));
    }

    #[test]
    fn new_borrowed_is_alias() {
        let data = b"alias check";
        let frame = Frame::new_borrowed(data);
        assert_eq!(frame.len(), data.len());
        assert_eq!(frame.payload(), data);
    }

    #[test]
    fn new_owned_copies() {
        let owned = b"owned payload".to_vec();
        let frame = Frame::new_owned(owned.clone());
        assert_eq!(frame.len(), owned.len());
        assert_eq!(frame.payload(), owned.as_slice());
        assert!(matches!(frame.data, Cow::Owned(_)));
    }

    #[test]
    fn from_slice_impl() {
        let data = b"from slice";
        let frame: Frame = data.as_slice().into();
        assert_eq!(frame.payload(), data);
    }

    #[test]
    fn empty_frame() {
        let frame = Frame::new(&[]);
        assert!(frame.is_empty());
        assert_eq!(frame.len(), 0);
        assert_eq!(frame.payload(), &[] as &[u8]);
    }

    #[test]
    fn deref_to_slice() {
        let data = b"deref";
        let frame = Frame::new(data);
        assert_eq!(&frame[..], data);
        assert_eq!(frame.len(), data.len());
    }
}
