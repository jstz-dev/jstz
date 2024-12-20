pub(crate) trait AsRawPtr {
    type Ptr;

    /// Get the raw pointer to the underlying object.
    unsafe fn as_raw_ptr(&self) -> Self::Ptr;
}
