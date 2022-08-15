#![cfg_attr(not(test), no_std)]

use core::str;

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

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
	UnterminatedQuote,
	InvalidSymbolChar,
	InvalidUtf8,
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

pub fn parse<'a>(data: &'a [u8]) -> Iter<'a> {
	Iter { data, index: 0 }
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
}
