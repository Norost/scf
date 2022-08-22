#![cfg_attr(not(test), no_std)]

use core::{cell::Cell, str};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Token<'a> {
	Begin,
	End,
	Str(&'a str),
}

impl<'a> Token<'a> {
	pub fn into_str(self) -> Option<&'a str> {
		match self {
			Self::Str(s) => Some(s),
			_ => None,
		}
	}
}

pub struct Iter<'a> {
	data: &'a [u8],
	index: usize,
}

impl<'a> Iterator for Iter<'a> {
	type Item = Result<Token<'a>, Error>;

	fn next(&mut self) -> Option<Self::Item> {
		let ret_str = |s| {
			str::from_utf8(s)
				.map_err(|_| Error::InvalidUtf8)
				.map(|s| Token::Str(s))
		};
		loop {
			let c = self.data.get(self.index)?;
			self.index += 1;
			match c {
				c if c.is_ascii_whitespace() => {}
				b'(' => return Some(Ok(Token::Begin)),
				b')' => return Some(Ok(Token::End)),
				lim @ b'"' | lim @ b'\'' => loop {
					let start = self.index;
					while let Some(&c) = self.data.get(self.index) {
						self.index += 1;
						match c {
							b'\\' => self.index += 1,
							c if c == *lim => {
								return Some(ret_str(&self.data[start..self.index - 1]));
							}
							_ => {}
						}
					}
					return Some(Err(Error::UnterminatedQuote));
				},
				b';' => {
					while self.data.get(self.index).map_or(false, |c| *c != b'\n') {
						self.index += 1;
					}
				}
				_ => loop {
					let start = self.index - 1;
					while let Some(&c) = self.data.get(self.index) {
						self.index += 1;
						match c {
							c if c == b'(' || c == b')' || c.is_ascii_whitespace() => {
								self.index -= 1;
								break;
							}
							_ => {}
						}
					}
					return Some(ret_str(&self.data[start..self.index]));
				},
			}
		}
	}
}

#[derive(Debug)]
#[must_use = "an error may have occured"]
pub struct Groups<'a> {
	data: &'a [u8],
	index: Cell<usize>,
}

impl<'a> Groups<'a> {
	pub fn iter(&mut self) -> GroupsIter<'a, '_> {
		GroupsIter { inner: Some(self) }
	}

	pub fn into_error(self) -> Option<Error> {
		Error::from_num(self.index.get())
	}
}

#[derive(Debug)]
pub struct GroupsIter<'a, 'b> {
	inner: Option<&'b Groups<'a>>,
}

impl<'a, 'b> GroupsIter<'a, 'b> {
	pub fn next_str(&mut self) -> Option<&'a str> {
		self.next().and_then(|e| e.into_str())
	}

	pub fn next_group(&mut self) -> Option<GroupsIter<'a, 'b>> {
		self.next().and_then(|e| e.into_group())
	}
}

impl<'a, 'b> Iterator for GroupsIter<'a, 'b> {
	type Item = Item<'a, 'b>;

	fn next(&mut self) -> Option<Self::Item> {
		let r = self.inner?;
		let mut it = Iter {
			data: r.data,
			index: r.index.get(),
		};
		if (it.index as isize) < 0 {
			return None;
		}
		let tk = it.next();
		r.index.set(it.index);
		match tk {
			None => None,
			Some(Err(e)) => {
				r.index.set(e.into_num());
				None
			}
			Some(Ok(tk)) => Some(match tk {
				Token::Str(s) => Item::Str(s),
				Token::Begin => Item::Group(Self { inner: self.inner }),
				Token::End => {
					self.inner = None;
					return None
				}
			}),
		}
	}
}

impl Drop for GroupsIter<'_, '_> {
	fn drop(&mut self) {
		for _ in self {}
	}
}

impl core::iter::FusedIterator for GroupsIter<'_, '_> {}

#[derive(Debug)]
pub enum Item<'a, 'b> {
	Str(&'a str),
	Group(GroupsIter<'a, 'b>),
}

impl<'a, 'b> Item<'a, 'b> {
	pub fn into_str(self) -> Option<&'a str> {
		match self {
			Self::Str(s) => Some(s),
			_ => None,
		}
	}

	pub fn into_group(self) -> Option<GroupsIter<'a, 'b>> {
		match self {
			Self::Group(g) => Some(g),
			_ => None,
		}
	}
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
	UnterminatedQuote,
	InvalidSymbolChar,
	InvalidUtf8,
}

impl Error {
	fn into_num(self) -> usize {
		(match self {
			Self::UnterminatedQuote => -1,
			Self::InvalidSymbolChar => -2,
			Self::InvalidUtf8 => -3,
		}) as usize
	}

	fn from_num(n: usize) -> Option<Self> {
		Some(match n as isize {
			-1 => Self::UnterminatedQuote,
			-2 => Self::InvalidSymbolChar,
			-3 => Self::InvalidUtf8,
			_ => return None,
		})
	}
}

#[deprecated(note = "use `parse2`, which is less error-prone")]
pub fn parse<'a>(data: &'a [u8]) -> Iter<'a> {
	Iter { data, index: 0 }
}

pub fn parse2<'a>(data: &'a [u8]) -> Groups<'a> {
	Groups { data, index: 0.into() }
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn example_pci() {
		let t = br#"(pci-drivers
	(1af4 ; Red Hat
		(1000 "drivers/pci/virtio/net")
		(1001 "drivers/pci/virtio/blk")
		(1050 "drivers/pci/virtio/gpu"))
	(8086 ; Intel
		(1616 "drivers/pci/intel/hd graphics"))) ; intentional space"#;
		#[allow(deprecated)]
		let mut it = parse(t);
		assert_eq!(it.next(), Some(Ok(Token::Begin)));
		assert_eq!(it.next(), Some(Ok(Token::Str("pci-drivers"))));
		assert_eq!(it.next(), Some(Ok(Token::Begin)));
		assert_eq!(it.next(), Some(Ok(Token::Str("1af4"))));
		assert_eq!(it.next(), Some(Ok(Token::Begin)));
		assert_eq!(it.next(), Some(Ok(Token::Str("1000"))));
		assert_eq!(it.next(), Some(Ok(Token::Str("drivers/pci/virtio/net"))));
		assert_eq!(it.next(), Some(Ok(Token::End)));
		assert_eq!(it.next(), Some(Ok(Token::Begin)));
		assert_eq!(it.next(), Some(Ok(Token::Str("1001"))));
		assert_eq!(it.next(), Some(Ok(Token::Str("drivers/pci/virtio/blk"))));
		assert_eq!(it.next(), Some(Ok(Token::End)));
		assert_eq!(it.next(), Some(Ok(Token::Begin)));
		assert_eq!(it.next(), Some(Ok(Token::Str("1050"))));
		assert_eq!(it.next(), Some(Ok(Token::Str("drivers/pci/virtio/gpu"))));
		assert_eq!(it.next(), Some(Ok(Token::End)));
		assert_eq!(it.next(), Some(Ok(Token::End)));
		assert_eq!(it.next(), Some(Ok(Token::Begin)));
		assert_eq!(it.next(), Some(Ok(Token::Str("8086"))));
		assert_eq!(it.next(), Some(Ok(Token::Begin)));
		assert_eq!(it.next(), Some(Ok(Token::Str("1616"))));
		assert_eq!(
			it.next(),
			Some(Ok(Token::Str("drivers/pci/intel/hd graphics")))
		);
		assert_eq!(it.next(), Some(Ok(Token::End)));
		assert_eq!(it.next(), Some(Ok(Token::End)));
		assert_eq!(it.next(), Some(Ok(Token::End)));
		assert_eq!(it.next(), None);
	}

	#[test]
	fn example_pci_groups() {
		let t = br#"(pci-drivers
	(1af4 ; Red Hat
		(1000 "drivers/pci/virtio/net")
		(1001 "drivers/pci/virtio/blk")
		(1050 "drivers/pci/virtio/gpu"))
	(8086 ; Intel
		(1616 "drivers/pci/intel/hd graphics"))) ; intentional space"#;
		#[track_caller]
		fn string<'a, 'b>(it: &mut GroupsIter<'a, 'b>) -> &'a str {
			it.next().unwrap().into_str().unwrap()
		}
		#[track_caller]
		fn group<'a, 'b>(it: &mut GroupsIter<'a, 'b>) -> GroupsIter<'a, 'b> {
			it.next().unwrap().into_group().unwrap()
		}
		#[track_caller]
		fn none<'a, 'b>(it: &mut GroupsIter<'a, 'b>) {
			assert!(it.next().is_none());
			// Multiple times as a sanity check
			assert!(it.next().is_none());
			assert!(it.next().is_none());
		}

		let mut cf = parse2(t);
		{
			let mut it = cf.iter();
			let mut it2 = group(&mut it);
			assert_eq!(string(&mut it2), "pci-drivers");
			let mut it3 = group(&mut it2);
			assert_eq!(string(&mut it3), "1af4");
			let mut it4 = group(&mut it3);
			assert_eq!(string(&mut it4), "1000");
			assert_eq!(string(&mut it4), "drivers/pci/virtio/net");
			none(&mut it4);
			let mut it4 = group(&mut it3);
			assert_eq!(string(&mut it4), "1001");
			assert_eq!(string(&mut it4), "drivers/pci/virtio/blk");
			none(&mut it4);
			let mut it4 = group(&mut it3);
			assert_eq!(string(&mut it4), "1050");
			assert_eq!(string(&mut it4), "drivers/pci/virtio/gpu");
			none(&mut it4);
			none(&mut it3);
			let mut it3 = group(&mut it2);
			assert_eq!(string(&mut it3), "8086");
			let mut it4 = group(&mut it3);
			assert_eq!(string(&mut it4), "1616");
			assert_eq!(string(&mut it4), "drivers/pci/intel/hd graphics");
			none(&mut it4);
			none(&mut it3);
			none(&mut it2);
			none(&mut it);
		}
		assert!(cf.into_error().is_none());
	}

	#[test]
	fn partial_iter_group() {
		let t = br#"(pci-drivers
	(1af4 ; Red Hat
		(1000 "drivers/pci/virtio/net")
		(1001 "drivers/pci/virtio/blk")
		(1050 "drivers/pci/virtio/gpu"))
	(8086 ; Intel
		(1616 "drivers/pci/intel/hd graphics"))) ; intentional space"#;
		#[track_caller]
		fn string<'a, 'b>(it: &mut GroupsIter<'a, 'b>) -> &'a str {
			it.next().unwrap().into_str().unwrap()
		}
		#[track_caller]
		fn group<'a, 'b>(it: &mut GroupsIter<'a, 'b>) -> GroupsIter<'a, 'b> {
			it.next().unwrap().into_group().unwrap()
		}
		#[track_caller]
		fn none<'a, 'b>(it: &mut GroupsIter<'a, 'b>) {
			assert!(it.next().is_none());
			// Multiple times as a sanity check
			assert!(it.next().is_none());
			assert!(it.next().is_none());
		}

		let mut cf = parse2(t);
		{
			let mut it = cf.iter();
			let mut it2 = group(&mut it);
			assert_eq!(string(&mut it2), "pci-drivers");
			let mut it3 = group(&mut it2);
			assert_eq!(string(&mut it3), "1af4");
			let mut it4 = group(&mut it3);
			assert_eq!(string(&mut it4), "1000");
			assert_eq!(string(&mut it4), "drivers/pci/virtio/net");
			none(&mut it4);
			let mut it4 = group(&mut it3);
			assert_eq!(string(&mut it4), "1001");
			drop(it4);
			drop(it3);
			let mut it3 = group(&mut it2);
			assert_eq!(string(&mut it3), "8086");
			let mut it4 = group(&mut it3);
			assert_eq!(string(&mut it4), "1616");
			assert_eq!(string(&mut it4), "drivers/pci/intel/hd graphics");
			none(&mut it4);
			none(&mut it3);
			none(&mut it2);
			none(&mut it);
		}
		assert!(cf.into_error().is_none());
	}
}
