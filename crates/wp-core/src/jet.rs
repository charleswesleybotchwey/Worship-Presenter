//! Read-only reader for Microsoft Access Jet 4 databases (.mdb, Bible Show .bib).
//!
//! Enough of the format to read every row of a named table: the system catalogue
//! (MSysObjects, table definition on page 2), table definitions that may span
//! several pages, data pages, overflow rows, compressed Unicode text and memo
//! fields stored inline or in long-value pages. No drivers, no helper programs.
//!
//! Layout notes follow the mdbtools format documentation (HACKING) for Jet 4:
//! 4096-byte pages; a data page starts 01 01, free space u16 @2, owning table
//! definition page u32 @4, row count u16 @12, row offsets u16 from @14 (0x8000
//! deleted, 0x4000 overflow pointer, low 13 bits offset). Rows are packed from the
//! end of the page backwards. A row is: column count u16, fixed columns, variable
//! columns, then from the end: null mask, variable column count u16, variable
//! column offsets u16 (stored backwards), end-of-data u16.

use crate::{Error, Result};
use serde_json::{Map, Number, Value};
use std::path::Path;

const PAGE: usize = 4096;

#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub kind: u8,
    pub num: u16,
    pub var_index: u16,
    pub fixed_offset: u16,
    pub len: u16,
    pub fixed: bool,
}

pub struct Database {
    data: Vec<u8>,
}

fn bad(why: impl Into<String>) -> Error {
    Error::Format("Access database".into(), "Jet 4", why.into())
}

fn u16_at(b: &[u8], o: usize) -> Result<u16> {
    b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| bad(format!("read past end at {o}")))
}
fn u32_at(b: &[u8], o: usize) -> Result<u32> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| bad(format!("read past end at {o}")))
}

/// Jet 4 text: UCS-2 little-endian, or "compressed" when it starts FF FE, where a
/// zero byte toggles between one-byte characters and two-byte characters.
pub fn decode_text(b: &[u8]) -> String {
    if b.len() >= 2 && b[0] == 0xFF && b[1] == 0xFE {
        let mut out = String::new();
        let mut compressed = true;
        let mut i = 2;
        while i < b.len() {
            if b[i] == 0 {
                compressed = !compressed;
                i += 1;
            } else if compressed {
                out.push(b[i] as char);
                i += 1;
            } else if i + 1 < b.len() {
                out.extend(char::decode_utf16([u16::from_le_bytes([b[i], b[i + 1]])]).map(|r| r.unwrap_or('\u{FFFD}')));
                i += 2;
            } else {
                break;
            }
        }
        out
    } else {
        let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        char::decode_utf16(units).map(|r| r.unwrap_or('\u{FFFD}')).collect()
    }
}

impl Database {
    pub fn open(path: &Path) -> Result<Database> {
        Self::from_bytes(std::fs::read(path)?)
    }

    pub fn from_bytes(data: Vec<u8>) -> Result<Database> {
        if data.len() < PAGE * 3 || &data[4..19] != b"Standard Jet DB" {
            return Err(bad("not a Jet database (missing 'Standard Jet DB' signature)"));
        }
        match data[0x14] {
            1 => {}
            0 => return Err(bad("Jet 3 (Access 97) files are not supported; save as Access 2000 or later")),
            v => return Err(bad(format!("Access 2007+ format (version byte {v}) is not supported yet"))),
        }
        Ok(Database { data })
    }

    fn page(&self, n: u32) -> Result<&[u8]> {
        let s = n as usize * PAGE;
        self.data.get(s..s + PAGE).ok_or_else(|| bad(format!("page {n} is past the end of the file")))
    }

    /// Table definition, joined across continuation pages (each skips an 8-byte header).
    fn tdef(&self, first: u32) -> Result<Vec<u8>> {
        let p = self.page(first)?;
        if p[0] != 0x02 {
            return Err(bad(format!("page {first} is not a table definition")));
        }
        let mut buf = p.to_vec();
        let mut next = u32_at(p, 4)?;
        let mut guard = 0;
        while next != 0 {
            let q = self.page(next)?;
            buf.extend_from_slice(&q[8..]);
            next = u32_at(q, 4)?;
            guard += 1;
            if guard > 1000 {
                return Err(bad("table definition chain loops"));
            }
        }
        Ok(buf)
    }

    pub fn columns(&self, tdef_page: u32) -> Result<Vec<Column>> {
        let t = self.tdef(tdef_page)?;
        let num_cols = u16_at(&t, 45)? as usize;
        let num_ridx = u32_at(&t, 51)? as usize;
        let mut pos = 63 + num_ridx * 12;
        let mut cols = Vec::with_capacity(num_cols);
        for _ in 0..num_cols {
            let e = t.get(pos..pos + 25).ok_or_else(|| bad("column list truncated"))?;
            cols.push(Column {
                name: String::new(),
                kind: e[0],
                num: u16::from_le_bytes([e[5], e[6]]),
                var_index: u16::from_le_bytes([e[7], e[8]]),
                fixed: e[15] & 0x01 != 0,
                fixed_offset: u16::from_le_bytes([e[21], e[22]]),
                len: u16::from_le_bytes([e[23], e[24]]),
            });
            pos += 25;
        }
        for c in cols.iter_mut() {
            let n = u16_at(&t, pos)? as usize;
            c.name = decode_text(t.get(pos + 2..pos + 2 + n).ok_or_else(|| bad("column name truncated"))?);
            pos += 2 + n;
        }
        Ok(cols)
    }

    /// (row bytes) for every live row of the table whose definition is on `tdef_page`.
    fn raw_rows(&self, tdef_page: u32) -> Result<Vec<Vec<u8>>> {
        let mut out = Vec::new();
        for n in 0..(self.data.len() / PAGE) as u32 {
            let p = self.page(n)?;
            if p[0] != 0x01 || p[1] != 0x01 || u32_at(p, 4)? != tdef_page {
                continue;
            }
            let count = u16_at(p, 12)? as usize;
            for i in 0..count {
                let raw = u16_at(p, 14 + i * 2)?;
                if raw & 0x8000 != 0 {
                    continue; // deleted
                }
                let (start, end) = self.row_bounds(p, i)?;
                let row = &p[start..end];
                if raw & 0x4000 != 0 {
                    // overflow: 4-byte pointer (row number, 3-byte page) to the real row
                    let ptr = u32_at(row, 0)?;
                    out.push(self.row_at(ptr)?.to_vec());
                } else {
                    out.push(row.to_vec());
                }
            }
        }
        Ok(out)
    }

    fn row_bounds(&self, p: &[u8], i: usize) -> Result<(usize, usize)> {
        let start = (u16_at(p, 14 + i * 2)? & 0x1FFF) as usize;
        let end = if i == 0 { PAGE } else { (u16_at(p, 14 + (i - 1) * 2)? & 0x1FFF) as usize };
        if start >= end || end > PAGE {
            return Err(bad(format!("row {i} has bad bounds {start}..{end}")));
        }
        Ok((start, end))
    }

    /// The row a long-value / overflow pointer names: low byte row, high 3 bytes page.
    fn row_at(&self, ptr: u32) -> Result<&[u8]> {
        let page = ptr >> 8;
        let row = (ptr & 0xFF) as usize;
        let p = self.page(page)?;
        let count = u16_at(p, 12)? as usize;
        if row >= count {
            return Err(bad(format!("pointer to row {row} on page {page}, which has {count} rows")));
        }
        let (s, e) = self.row_bounds(p, row)?;
        Ok(&p[s..e])
    }

    /// Memo / OLE field: 12-byte header, then inline data or a chain of long-value rows.
    fn memo(&self, field: &[u8]) -> Result<Vec<u8>> {
        if field.len() < 12 {
            return Ok(Vec::new());
        }
        let raw_len = u32_at(field, 0)?;
        let len = (raw_len & 0x3FFF_FFFF) as usize;
        if raw_len & 0x8000_0000 != 0 {
            return Ok(field[12..(12 + len).min(field.len())].to_vec());
        }
        let ptr = u32_at(field, 4)?;
        if raw_len & 0x4000_0000 != 0 {
            let r = self.row_at(ptr)?;
            return Ok(r[..len.min(r.len())].to_vec());
        }
        let mut out = Vec::with_capacity(len);
        let mut next = ptr;
        let mut guard = 0;
        while next != 0 && out.len() < len {
            let r = self.row_at(next)?;
            next = u32_at(r, 0)?;
            out.extend_from_slice(&r[4..]);
            guard += 1;
            if guard > 100_000 {
                return Err(bad("memo chain loops"));
            }
        }
        out.truncate(len);
        Ok(out)
    }

    fn decode_row(&self, row: &[u8], cols: &[Column]) -> Result<Map<String, Value>> {
        let total = cols.len();
        let row_cols = u16_at(row, 0)? as usize;
        let mask_len = row_cols.div_ceil(8);
        if row.len() < 2 + mask_len + 4 {
            return Err(bad("row too short"));
        }
        let mask = &row[row.len() - mask_len..];
        let last = row.len() - 1; // index of the final byte
        let var_count = u16_at(row, last - mask_len - 1)? as usize;
        let mut var_offsets = Vec::with_capacity(var_count + 1);
        for i in 0..=var_count {
            var_offsets.push(u16_at(row, last - mask_len - 3 - i * 2)? as usize);
        }
        let mut m = Map::new();
        for c in cols {
            let n = c.num as usize;
            let present = n < row_cols && mask.get(n / 8).map_or(false, |b| b & (1 << (n % 8)) != 0);
            if c.kind == 1 {
                m.insert(c.name.clone(), Value::Bool(present));
                continue;
            }
            if !present || (!c.fixed && c.var_index as usize >= var_count) {
                continue; // NULL: left out, like mdb-json
            }
            let bytes: &[u8] = if c.fixed {
                let s = 2 + c.fixed_offset as usize;
                row.get(s..s + c.len as usize).ok_or_else(|| bad(format!("{} runs past row", c.name)))?
            } else {
                let vi = c.var_index as usize;
                let (s, e) = (var_offsets[vi], var_offsets[vi + 1]);
                row.get(s..e).ok_or_else(|| bad(format!("{} has bad offsets", c.name)))?
            };
            let v = match c.kind {
                2 => Value::from(bytes[0]),
                3 => Value::from(i16::from_le_bytes([bytes[0], bytes[1]])),
                4 => Value::from(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
                6 => Number::from_f64(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f64).map_or(Value::Null, Value::Number),
                5 | 7 | 8 => {
                    let x = f64::from_le_bytes(bytes[..8].try_into().unwrap_or([0; 8]));
                    let x = if c.kind == 5 { i64::from_le_bytes(bytes[..8].try_into().unwrap_or([0; 8])) as f64 / 10_000.0 } else { x };
                    Number::from_f64(x).map_or(Value::Null, Value::Number)
                }
                10 => Value::String(decode_text(bytes)),
                12 => Value::String(decode_text(&self.memo(bytes)?)),
                _ => continue, // binary, OLE, GUID, numeric: not needed for Bibles
            };
            m.insert(c.name.clone(), v);
        }
        let _ = total;
        Ok(m)
    }

    /// Names and definition pages of the user tables.
    pub fn tables(&self) -> Result<Vec<(String, u32)>> {
        let cols = self.columns(2)?;
        let mut out = Vec::new();
        for r in self.raw_rows(2)? {
            let m = self.decode_row(&r, &cols)?;
            let ty = m.get("Type").and_then(Value::as_i64).unwrap_or(0);
            let flags = m.get("Flags").and_then(Value::as_i64).unwrap_or(0) as u32;
            let id = m.get("Id").and_then(Value::as_i64).unwrap_or(0) as u32;
            let name = m.get("Name").and_then(Value::as_str).unwrap_or_default().to_string();
            if ty == 1 && flags & 0x8000_0002 == 0 && !name.starts_with("MSys") {
                out.push((name, id & 0x00FF_FFFF));
            }
        }
        out.sort();
        Ok(out)
    }

    /// Every live row of a user table as column → value; NULL columns are omitted.
    pub fn rows(&self, table: &str) -> Result<Vec<Map<String, Value>>> {
        let (_, page) = self
            .tables()?
            .into_iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(table))
            .ok_or_else(|| bad(format!("no table named {table}")))?;
        let cols = self.columns(page)?;
        self.raw_rows(page)?.iter().map(|r| self.decode_row(r, &cols)).collect()
    }
}

impl crate::import::bibleshow::Source for Database {
    fn rows(&self, table: &str) -> Result<Vec<Map<String, Value>>> {
        Database::rows(self, table)
    }
}

#[cfg(test)]
mod tests {
    use super::decode_text;

    #[test]
    fn compressed_and_plain_text() {
        // FF FE, "Ab" compressed, toggle, "ɛ" as UCS-2, toggle, "c"
        let b = [0xFF, 0xFE, b'A', b'b', 0x00, 0x5B, 0x02, 0x00, b'c'];
        assert_eq!(decode_text(&b), "Abɛc");
        assert_eq!(decode_text(&[0x4F, 0x00, 0x54, 0x02]), "Oɔ");
    }

    #[test]
    fn rejects_non_jet() {
        assert!(super::Database::from_bytes(vec![0; 16384]).is_err());
    }
}
