use std::ptr;

pub struct StaticPointer<T> {
	#[cfg(debug_assertions)]
	not_null: bool,
	ptr: *const T,
}

impl<T> StaticPointer<T> {
	pub const fn null() -> Self {
		Self {
			#[cfg(debug_assertions)]
			not_null: false,
			ptr: ptr::null()
		}
	}

	pub const fn new(ptr: *const T) -> Self {
		Self {
			#[cfg(debug_assertions)]
			not_null: true,
			ptr
		}
	}

	/// SAFETY: The caller must make sure the pointer is still valid.
	pub const unsafe fn deref(&self) -> &T {
		#[cfg(debug_assertions)]
		assert!(self.not_null);
		&*self.ptr	
	}

	pub fn set(&mut self, new_ptr: *const T) {
		#[cfg(debug_assertions)]
		{ self.not_null = !new_ptr.is_null(); }
		self.ptr = new_ptr;
	}

	pub fn is_null(&self) -> bool {
		self.ptr.is_null()
	}
}

unsafe impl<T> Send for StaticPointer<T> {}
