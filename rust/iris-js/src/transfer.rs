//! The binary layout that carries data from WebAssembly into JavaScript.
//!
//! Every number is little-endian. Unsigned integers are `u32`, and each
//! numeric array starts at an 8-byte boundary of the buffer, so JavaScript can
//! view it in place as a `BigInt64Array`, `Float64Array` or `Uint32Array`.
//! Byte strings are padded with zeros to a 4-byte boundary.
//!
//! ```text
//! data   := FORMAT_VERSION entry-count entry*
//! entry  := name-length name-bytes value-type depth level{depth} leaves
//! level  := offset-count pad8 offset{offset-count}
//! leaves := leaf-count pad8 i64{leaf-count}                    value type 0, int
//!         | leaf-count pad8 f64{leaf-count}                    value type 1, float
//!         | leaf-count pad8 u32{leaf-count + 1} string-bytes   value type 2, string
//! ```
//!
//! Levels hold the cumulative offsets of `iris_core::Value::levels`. String
//! leaves are byte offsets into the UTF-8 bytes that follow them; the last
//! offset is the byte count.
//!
//! `typescript/src/transfer.ts` decodes the same layout.
use iris_core::{ColumnError, Data, Entry, Leaves, Value};
use std::fmt;

pub const FORMAT_VERSION: u32 = 1;
const INT: u32 = 0;
const FLOAT: u32 = 1;
const STRING: u32 = 2;

/// Data that cannot be encoded, or bytes that do not decode to valid data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferError {
    /// A count, offset or length does not fit in 32 bits.
    TooLarge,
    /// The bytes end before the layout does.
    Truncated,
    /// Bytes remain after the last entry.
    TrailingBytes,
    UnknownVersion(u32),
    UnknownValueType(u32),
    /// A name or string leaf is not UTF-8, or string offsets are not valid.
    InvalidString,
    Column(ColumnError),
}
impl fmt::Display for TransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => f.write_str("data exceeds the 32-bit transfer limits"),
            Self::Truncated => f.write_str("transfer buffer is truncated"),
            Self::TrailingBytes => f.write_str("transfer buffer has trailing bytes"),
            Self::UnknownVersion(version) => write!(f, "unknown transfer format {version}"),
            Self::UnknownValueType(tag) => write!(f, "unknown value type {tag}"),
            Self::InvalidString => f.write_str("transfer buffer has an invalid string"),
            Self::Column(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for TransferError {}
impl From<ColumnError> for TransferError {
    fn from(error: ColumnError) -> Self {
        Self::Column(error)
    }
}

pub fn encode(data: &Data) -> Result<Vec<u8>, TransferError> {
    let mut out = Writer::default();
    out.u32(FORMAT_VERSION);
    out.count(data.entries().len())?;
    for entry in data.entries() {
        out.bytes(entry.name().as_bytes())?;
        let value = entry.value();
        let tag = match value.leaves() {
            Leaves::Int(_) => INT,
            Leaves::Float(_) => FLOAT,
            Leaves::String(_) => STRING,
        };
        out.u32(tag);
        out.count(value.depth())?;
        for offsets in value.levels() {
            out.count(offsets.len())?;
            out.align8();
            for &offset in offsets {
                out.count(offset)?;
            }
        }
        match value.leaves() {
            Leaves::Int(leaves) => {
                out.count(leaves.len())?;
                out.align8();
                leaves.iter().for_each(|leaf| out.raw(&leaf.to_le_bytes()));
            }
            Leaves::Float(leaves) => {
                out.count(leaves.len())?;
                out.align8();
                leaves.iter().for_each(|leaf| out.raw(&leaf.to_le_bytes()));
            }
            Leaves::String(leaves) => {
                out.count(leaves.len())?;
                out.align8();
                let mut end = 0usize;
                out.u32(0);
                for leaf in leaves {
                    end += leaf.len();
                    out.count(end)?;
                }
                leaves.iter().for_each(|leaf| out.raw(leaf.as_bytes()));
                out.align4();
            }
        }
    }
    Ok(out.buffer)
}

pub fn decode(bytes: &[u8]) -> Result<Data, TransferError> {
    let mut input = Reader { bytes, position: 0 };
    let version = input.u32()?;
    if version != FORMAT_VERSION {
        return Err(TransferError::UnknownVersion(version));
    }
    let entry_count = input.count()?;
    let mut entries = Vec::new();
    for _ in 0..entry_count {
        let name = input.string()?;
        let tag = input.u32()?;
        let depth = input.count()?;
        let mut levels = Vec::new();
        for _ in 0..depth {
            let offset_count = input.count()?;
            input.align8()?;
            levels.push(input.offsets(offset_count)?);
        }
        let leaf_count = input.count()?;
        input.align8()?;
        let value = match tag {
            INT => Value::from_ints(input.array(leaf_count, i64::from_le_bytes)?, levels)?,
            FLOAT => Value::from_floats(input.array(leaf_count, f64::from_le_bytes)?, levels)?,
            STRING => Value::from_strings(input.strings(leaf_count)?, levels)?,
            tag => return Err(TransferError::UnknownValueType(tag)),
        };
        entries.push(Entry::new(name, value));
    }
    if input.position != bytes.len() {
        return Err(TransferError::TrailingBytes);
    }
    Ok(Data::from_entries(entries)?)
}

#[derive(Default)]
struct Writer {
    buffer: Vec<u8>,
}
impl Writer {
    fn raw(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }
    fn u32(&mut self, value: u32) {
        self.raw(&value.to_le_bytes());
    }
    fn count(&mut self, value: usize) -> Result<(), TransferError> {
        self.u32(u32::try_from(value).map_err(|_| TransferError::TooLarge)?);
        Ok(())
    }
    /// A length-prefixed byte string, padded to 4 bytes.
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), TransferError> {
        self.count(bytes.len())?;
        self.raw(bytes);
        self.align4();
        Ok(())
    }
    fn align4(&mut self) {
        self.buffer.resize(self.buffer.len().next_multiple_of(4), 0);
    }
    fn align8(&mut self) {
        self.buffer.resize(self.buffer.len().next_multiple_of(8), 0);
    }
}

/// Checked reads: every read reports truncation instead of panicking.
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], TransferError> {
        let end = self
            .position
            .checked_add(length)
            .filter(|&end| end <= self.bytes.len())
            .ok_or(TransferError::Truncated)?;
        let taken = &self.bytes[self.position..end];
        self.position = end;
        Ok(taken)
    }
    fn chunk<const N: usize>(&mut self) -> Result<[u8; N], TransferError> {
        let mut chunk = [0; N];
        chunk.copy_from_slice(self.take(N)?);
        Ok(chunk)
    }
    fn u32(&mut self) -> Result<u32, TransferError> {
        Ok(u32::from_le_bytes(self.chunk()?))
    }
    fn count(&mut self) -> Result<usize, TransferError> {
        usize::try_from(self.u32()?).map_err(|_| TransferError::TooLarge)
    }
    fn align(&mut self, alignment: usize) -> Result<(), TransferError> {
        let padding = self.position.next_multiple_of(alignment) - self.position;
        self.take(padding).map(|_| ())
    }
    fn align8(&mut self) -> Result<(), TransferError> {
        self.align(8)
    }
    /// A length-prefixed UTF-8 string, padded to 4 bytes.
    fn string(&mut self) -> Result<String, TransferError> {
        let length = self.count()?;
        let text = std::str::from_utf8(self.take(length)?)
            .map_err(|_| TransferError::InvalidString)?
            .to_owned();
        self.align(4)?;
        Ok(text)
    }
    /// Reads `count` values, checking the length before allocating.
    fn array<const N: usize, T>(
        &mut self,
        count: usize,
        convert: fn([u8; N]) -> T,
    ) -> Result<Vec<T>, TransferError> {
        let bytes = self.take(count.checked_mul(N).ok_or(TransferError::Truncated)?)?;
        Ok(bytes
            .as_chunks::<N>()
            .0
            .iter()
            .map(|&chunk| convert(chunk))
            .collect())
    }
    fn offsets(&mut self, count: usize) -> Result<Vec<usize>, TransferError> {
        self.array(count, u32::from_le_bytes)?
            .into_iter()
            .map(|offset| usize::try_from(offset).map_err(|_| TransferError::TooLarge))
            .collect()
    }
    fn strings(&mut self, count: usize) -> Result<Vec<String>, TransferError> {
        let offsets = self.offsets(count.checked_add(1).ok_or(TransferError::Truncated)?)?;
        let total = *offsets.last().expect("count + 1 offsets");
        if offsets[0] != 0 || offsets.windows(2).any(|pair| pair[0] > pair[1]) {
            return Err(TransferError::InvalidString);
        }
        let blob = self.take(total)?;
        let strings = offsets
            .windows(2)
            .map(|pair| {
                std::str::from_utf8(&blob[pair[0]..pair[1]])
                    .map(str::to_owned)
                    .map_err(|_| TransferError::InvalidString)
            })
            .collect::<Result<_, _>>()?;
        self.align(4)?;
        Ok(strings)
    }
}
