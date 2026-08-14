// raw_string::raw_str_imp

use std::{
	cmp::Ordering,
	fmt::{self, Write},
	ops::{Deref, DerefMut},
	str::Utf8Error,
};

/// A borrowed string slice that may or may not contain valid UTF-8.
/// 
/// [`RawStr`] serves as an alternative to Rust's [`str`] type
/// that allows for arbitrary byte sequences,
/// including those that are not valid UTF-8.
/// 
/// [`RawStr`] is implemented as a wrapper around, and implements [`Deref`] + [`DerefMut`] to, `[u8]`.
/// Therefore, all methods available on `[u8]` are also available on [`RawStr`].
/// 
/// # Examples
/// 
/// Basic usage:
/// ```
/// # use rawstring::RawStr;
/// let str = "Hello, world!";
/// let raw = RawStr::new(str);
/// assert_eq!(raw, str);
/// ```
#[repr(transparent)]
#[derive(Eq, Hash)] // PartialEq, PartialOrd, Ord implemented manually
pub struct RawStr(pub [u8]);

impl RawStr {
	/// Returns a reference to a [`RawStr`] from any type
	/// that can be referenced as a byte slice.
	/// 
	/// # Examples
	/// ```
	/// # use rawstring::RawStr;
	/// let raw: &RawStr = RawStr::new("Hello, world!");
	/// assert_eq!(raw, b"Hello, world!");
	/// ```
	#[inline]
	#[must_use]
	pub fn new<B>(bytes: &B) -> &Self
	where
		B: ?Sized + AsRef<[u8]>
	{
		Self::from_bytes(bytes.as_ref())
	}

	/// Returns a mutable reference to a [`RawStr`] from any type
	/// that can be mutably referenced as a byte slice.
	/// 
	/// # Examples
	/// ```
	/// # use rawstring::RawStr;
	/// let mut data = *b"hello";
	/// let raw = RawStr::new_mut(&mut data);
	/// raw[0] = b'H';
	/// assert_eq!(&data, b"Hello");
	/// ```
	#[inline]
	#[must_use]
	pub fn new_mut<B>(b: &mut B) -> &mut Self
	where
		B: ?Sized + AsMut<[u8]>
	{
		Self::from_bytes_mut(b.as_mut())
	}

	/// Reinterprets the given bytes as a [`RawStr`].
	#[doc(hidden)]
	#[inline]
	#[must_use]
	pub const fn from_bytes(bytes: &[u8]) -> &Self {
		// SAFETY: RawStr is a transparent wrapper over [u8]
		unsafe { &*(bytes as *const [u8] as *const RawStr) }
	}

	/// Reinterprets the given mutable bytes slice as a mutable [`RawStr`].
	#[doc(hidden)]
	#[inline]
	#[must_use]
	pub const fn from_bytes_mut(bytes: &mut [u8]) -> &mut Self {
		// SAFETY: RawStr is a transparent wrapper over [u8]
		unsafe { &mut *(bytes as *mut [u8] as *mut RawStr) }
	}

	/// Converts the [`RawStr`] to a [`str`] if it contains valid UTF-8.
	/// Returns a [`Utf8Error`] if the bytes are not valid UTF-8.
	/// 
	/// See [`str::from_utf8`].
	/// 
	/// # Examples
	/// ```
	/// # use rawstring::RawStr;
	/// let good = RawStr::new("Hello, world!");
	/// assert!(good.to_utf8_checked().is_ok());
	/// 
	/// let bad = RawStr::new(b"abc\xFFabc");
	/// assert!(bad.to_utf8_checked().is_err());
	/// ```
	#[inline]
	#[must_use]
	pub const fn to_utf8_checked(&self) -> Result<&str, Utf8Error> {
		str::from_utf8(&self.0)
	}

	/// Returns `true` if the [`RawStr`] contains valid UTF-8.
	/// 
	/// # Examples
	/// ```
	/// # use rawstring::RawStr;
	/// let good = RawStr::new("Hello, world!");
	/// assert!(good.is_utf8());
	/// 
	/// let bad = RawStr::new(b"abc\xFFabc");
	/// assert!(!bad.is_utf8());
	/// ```
	#[inline]
	#[must_use]
	pub const fn is_utf8(&self) -> bool {
		self.to_utf8_checked().is_ok()
	}
}

impl Deref for RawStr {
	type Target = [u8];
	
	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for RawStr {
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl AsRef<[u8]> for RawStr {
	#[inline]
	fn as_ref(&self) -> &[u8] {
		&self.0
	}
}

impl AsMut<[u8]> for RawStr {
	#[inline]
	fn as_mut(&mut self) -> &mut [u8] {
		&mut self.0
	}
}

impl fmt::Debug for RawStr {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "\"")?;
		for chunk in self.utf8_chunks() {
			for c in chunk.valid().chars() {
				match c {
					'\0' => write!(f, "\\0")?,
					'\x01'..='\x7F' => write!(f, "{}", (c as u8).escape_ascii())?,
					_ => write!(f, "{}", c.escape_debug())?,
				}
			}
			write!(f, "{}", chunk.invalid().escape_ascii())?;
		}
		write!(f, "\"")?;
		Ok(())
	}
}

impl fmt::Display for RawStr {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fn fmt_no_pad(this: &RawStr, f: &mut fmt::Formatter<'_>) -> fmt::Result {
			// formats the bytes as utf8 without any padding
			// invalid utf8 chunks are replaced with the replacement character
			for chunk in this.utf8_chunks() {
				f.write_str(chunk.valid())?;
				if !chunk.invalid().is_empty() {
					f.write_char(crate::UNICODE_REPLACEMENT_CHARACTER)?;
				}
			}
			Ok(())
		}

		if let Some(align) = f.align() {
			// calculate the padding on both sides
			let len: usize = self
				.utf8_chunks()
				.map(|chunk| {
					chunk.valid().chars().count()
					+ if chunk.invalid().is_empty() { 0 } else { 1 }
				})
				.sum();
			let total_padding = f.width()
				.unwrap_or(0)
				.saturating_sub(len);
			let fill = f.fill();
			let (lpad, rpad) = match align {
				fmt::Alignment::Left => (0, total_padding),
				fmt::Alignment::Right => (total_padding, 0),
				fmt::Alignment::Center => {
					let half = total_padding / 2;
					(half, half + total_padding % 2)
				}
			};

			// write the padding and the formatted bytes
			for _ in 0..lpad {
				f.write_char(fill)?;
			}
			fmt_no_pad(self, f)?;
			for _ in 0..rpad {
				f.write_char(fill)?;
			}

			Ok(())
		} else {
			// no padding needed
			// directly format the bytes
			fmt_no_pad(self, f)
		}
	}
}

impl<T: ?Sized + AsRef<[u8]>> PartialEq<T> for RawStr {
	#[inline]
	fn eq(&self, other: &T) -> bool {
		&self.0 == other.as_ref()
	}
}

impl<T: ?Sized + AsRef<[u8]>> PartialOrd<T> for RawStr {
	#[inline]
	fn partial_cmp(&self, other: &T) -> Option<Ordering> {
		Some(self.0.cmp(other.as_ref()))
	}
}

impl Ord for RawStr {
	#[inline]
	fn cmp(&self, other: &Self) -> Ordering {
		self.0.cmp(&other.0)
	}
}

impl<'a> TryFrom<&'a RawStr> for &'a str {
	type Error = Utf8Error;

	#[inline]
	fn try_from(this: &'a RawStr) -> Result<Self, Self::Error> {
		this.to_utf8_checked()
	}
}