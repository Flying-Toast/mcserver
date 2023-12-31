use crate::proto::*;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{Read, Write};

#[derive(Debug, Clone)]
pub struct CompoundNbt<'a> {
    name: Cow<'a, str>,
    props: HashMap<Cow<'a, str>, Cow<'a, Nbt<'a>>>,
}

impl<'a> CompoundNbt<'a> {
    pub fn new(name: impl Into<Cow<'a, str>>) -> Self {
        Self {
            name: name.into(),
            props: HashMap::new(),
        }
    }

    pub fn set<P: Into<Cow<'a, str>>, V: Into<Cow<'a, Nbt<'a>>>>(&mut self, name: P, value: V) {
        self.props.insert(name.into(), value.into());
    }

    pub fn get<'b>(&'b self, name: &str) -> Option<&'b Nbt<'a>> {
        self.props.get(name).map(Borrow::borrow)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn props(&self) -> impl Iterator<Item = (&str, &Nbt<'a>)> {
        self.props.iter().map(|(a, b)| (a.borrow(), b.borrow()))
    }
}

#[derive(Debug, Clone)]
pub enum NbtList<'a> {
    Compound(Cow<'a, [CompoundNbt<'a>]>),
    Byte(Cow<'a, [i8]>),
    Short(Cow<'a, [i16]>),
    Int(Cow<'a, [i32]>),
    Long(Cow<'a, [i64]>),
    Float(Cow<'a, [f32]>),
    Double(Cow<'a, [f64]>),
    String(Cow<'a, [Cow<'a, str>]>),
}

#[derive(Debug, Clone)]
pub enum Nbt<'a> {
    Compound(CompoundNbt<'a>),
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Cow<'a, [i8]>),
    String(Cow<'a, str>),
    List(NbtList<'a>),
    IntArray(Cow<'a, [i32]>),
    LongArray(Cow<'a, [i64]>),
}

impl Nbt<'static> {
    /// Reads a full nbt. This is to be called to parse the entire nbt from the root, which is always a compound.
    /// As such, this is the only publicly visible nbt read method.
    pub fn read_compound<R: Read>(r: &mut R) -> CompoundNbt<'static> {
        let ttype = read_tagtype(r);
        // 10 = TAG_Compound
        if ttype != TagType::Compound {
            panic!("Expected tag type Compound, got tag type '{ttype:?}'");
        }

        let compound_name = read_ushort_string(r);

        let mut compound = CompoundNbt::new(compound_name);

        loop {
            let tagid = read_tagtype(r);
            if tagid == TagType::End {
                return compound;
            }

            let elem_name = read_ushort_string(r);
            let elem = read_nbt(r, tagid);

            compound.set(elem_name, elem);
        }
    }
}

impl<'a> From<&'a Nbt<'a>> for Cow<'a, Nbt<'a>> {
    fn from(value: &'a Nbt<'a>) -> Self {
        Self::Borrowed(value)
    }
}

impl<'a> From<Nbt<'a>> for Cow<'a, Nbt<'a>> {
    fn from(value: Nbt<'a>) -> Self {
        Self::Owned(value)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(i8)]
enum TagType {
    End = 0,
    Byte = 1,
    Short = 2,
    Int = 3,
    Long = 4,
    Float = 5,
    Double = 6,
    ByteArray = 7,
    String = 8,
    List = 9,
    Compound = 10,
    IntArray = 11,
    LongArray = 12,
}

fn read_nbt<R: Read>(r: &mut R, tag: TagType) -> Nbt<'static> {
    match tag {
        TagType::Byte => Nbt::Byte(read_byte(r)),
        TagType::Short => Nbt::Short(read_short(r)),
        TagType::Int => Nbt::Int(read_int(r)),
        TagType::Long => Nbt::Long(read_long(r)),
        TagType::Float => Nbt::Float(read_float(r)),
        TagType::Double => Nbt::Double(read_double(r)),
        // TODO: refactor the copy-paste between ByteArray, IntArray, LongArray
        TagType::ByteArray => {
            let len = read_int(r);
            assert!(len >= 0, "len < 0 :(");
            let len: usize = len.try_into().unwrap();
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(read_byte(r));
            }
            Nbt::ByteArray(Cow::Owned(arr))
        }
        TagType::String => Nbt::String(read_ushort_string(r).into()),
        TagType::List => {
            let list_type = read_tagtype(r);
            let len = read_int(r);

            // TODO: this is awful. fix all the copy-paste
            Nbt::List(match list_type {
                TagType::String => {
                    let mut arr = Vec::with_capacity(len.try_into().unwrap());
                    if len > 0 {
                        for _ in 0..len {
                            arr.push(Cow::Owned(read_ushort_string(r)));
                        }
                    }
                    NbtList::String(Cow::Owned(arr))
                }
                TagType::Compound => {
                    let mut arr = Vec::with_capacity(len.try_into().unwrap());
                    if len > 0 {
                        for _ in 0..len {
                            arr.push(Nbt::read_compound(r));
                        }
                    }
                    NbtList::Compound(Cow::Owned(arr))
                }
                TagType::Int => {
                    let mut arr = Vec::with_capacity(len.try_into().unwrap());
                    if len > 0 {
                        for _ in 0..len {
                            arr.push(read_int(r));
                        }
                    }
                    NbtList::Int(Cow::Owned(arr))
                }
                TagType::Long => {
                    let mut arr = Vec::with_capacity(len.try_into().unwrap());
                    if len > 0 {
                        for _ in 0..len {
                            arr.push(read_long(r));
                        }
                    }
                    NbtList::Long(Cow::Owned(arr))
                }
                TagType::Short => {
                    let mut arr = Vec::with_capacity(len.try_into().unwrap());
                    if len > 0 {
                        for _ in 0..len {
                            arr.push(read_short(r));
                        }
                    }
                    NbtList::Short(Cow::Owned(arr))
                }
                TagType::Byte => {
                    let mut arr = Vec::with_capacity(len.try_into().unwrap());
                    if len > 0 {
                        for _ in 0..len {
                            arr.push(read_byte(r));
                        }
                    }
                    NbtList::Byte(Cow::Owned(arr))
                }
                TagType::Double => {
                    let mut arr = Vec::with_capacity(len.try_into().unwrap());
                    if len > 0 {
                        for _ in 0..len {
                            arr.push(read_double(r));
                        }
                    }
                    NbtList::Double(Cow::Owned(arr))
                }
                TagType::Float => {
                    let mut arr = Vec::with_capacity(len.try_into().unwrap());
                    if len > 0 {
                        for _ in 0..len {
                            arr.push(read_float(r));
                        }
                    }
                    NbtList::Float(Cow::Owned(arr))
                }
                x => todo!("implement nbt parsing for lists of {x:?}"),
            })
        }
        TagType::Compound => Nbt::Compound(Nbt::read_compound(r)),
        TagType::IntArray => {
            let len = read_int(r);
            assert!(len >= 0, "len < 0 :(");
            let len: usize = len.try_into().unwrap();
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(read_int(r));
            }
            Nbt::IntArray(Cow::Owned(arr))
        }
        TagType::LongArray => {
            let len = read_int(r);
            assert!(len >= 0, "len < 0 :(");
            let len: usize = len.try_into().unwrap();
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(read_long(r));
            }
            Nbt::LongArray(Cow::Owned(arr))
        }
        TagType::End => panic!("can't read_nbt() with TagType::End"),
    }
}

fn read_tagtype<R: Read>(r: &mut R) -> TagType {
    use TagType::*;
    match read_byte(r) {
        0 => End,
        1 => Byte,
        2 => Short,
        3 => Int,
        4 => Long,
        5 => Float,
        6 => Double,
        7 => ByteArray,
        8 => String,
        9 => List,
        10 => Compound,
        11 => IntArray,
        12 => LongArray,
        x => panic!("bad tag type {x}"),
    }
}

fn write_tagtype<W: Write>(w: &mut W, tt: TagType) {
    write_ibyte(w, tt as i8);
}

fn write_compound_nbt_no_tagtype<W: Write>(w: &mut W, nbt: &CompoundNbt<'_>) {
    write_ushort_string(w, &nbt.name);
    for (prop_name, prop_value) in nbt.props() {
        match prop_value {
            Nbt::Compound(c) => write_compound_nbt(w, c),
            Nbt::String(s) => {
                write_tagtype(w, TagType::String);
                write_ushort_string(w, prop_name);
                write_ushort_string(w, s);
            }
            Nbt::Byte(b) => {
                write_tagtype(w, TagType::Byte);
                write_ushort_string(w, prop_name);
                write_ibyte(w, *b);
            }
            Nbt::Short(s) => {
                write_tagtype(w, TagType::Short);
                write_ushort_string(w, prop_name);
                write_short(w, *s);
            }
            Nbt::Int(i) => {
                write_tagtype(w, TagType::Int);
                write_ushort_string(w, prop_name);
                write_int(w, *i);
            }
            Nbt::Long(x) => {
                write_tagtype(w, TagType::Long);
                write_ushort_string(w, prop_name);
                write_long(w, *x);
            }
            Nbt::Float(x) => {
                write_tagtype(w, TagType::Float);
                write_ushort_string(w, prop_name);
                write_float(w, *x);
            }
            Nbt::Double(x) => {
                write_tagtype(w, TagType::Double);
                write_ushort_string(w, prop_name);
                write_double(w, *x);
            }
            Nbt::List(l) => {
                write_tagtype(w, TagType::List);
                write_ushort_string(w, prop_name);
                match l {
                    NbtList::Compound(c) => {
                        write_tagtype(w, TagType::Compound);
                        write_int(w, c.len().try_into().unwrap());
                        for x in c.iter() {
                            write_compound_nbt_no_tagtype(w, x);
                        }
                    }
                    NbtList::Byte(lst) => {
                        write_tagtype(w, TagType::Byte);
                        write_int(w, lst.len().try_into().unwrap());
                        for x in lst.iter() {
                            write_ibyte(w, *x);
                        }
                    }
                    NbtList::Short(lst) => {
                        write_tagtype(w, TagType::Short);
                        write_int(w, lst.len().try_into().unwrap());
                        for x in lst.iter() {
                            write_short(w, *x);
                        }
                    }
                    NbtList::Int(lst) => {
                        write_tagtype(w, TagType::Int);
                        write_int(w, lst.len().try_into().unwrap());
                        for x in lst.iter() {
                            write_int(w, *x);
                        }
                    }
                    NbtList::Long(lst) => {
                        write_tagtype(w, TagType::Long);
                        write_int(w, lst.len().try_into().unwrap());
                        for x in lst.iter() {
                            write_long(w, *x);
                        }
                    }
                    NbtList::Float(lst) => {
                        write_tagtype(w, TagType::Float);
                        write_int(w, lst.len().try_into().unwrap());
                        for x in lst.iter() {
                            write_float(w, *x);
                        }
                    }
                    NbtList::Double(lst) => {
                        write_tagtype(w, TagType::Double);
                        write_int(w, lst.len().try_into().unwrap());
                        for x in lst.iter() {
                            write_double(w, *x);
                        }
                    }
                    NbtList::String(lst) => {
                        write_tagtype(w, TagType::String);
                        write_int(w, lst.len().try_into().unwrap());
                        for x in lst.iter() {
                            write_ushort_string(w, x);
                        }
                    }
                }
            }
            Nbt::ByteArray(arr) => {
                write_tagtype(w, TagType::ByteArray);
                write_ushort_string(w, prop_name);
                write_int(w, arr.len().try_into().unwrap());
                for b in arr.iter().copied() {
                    write_ibyte(w, b);
                }
            }
            Nbt::IntArray(arr) => {
                write_tagtype(w, TagType::IntArray);
                write_ushort_string(w, prop_name);
                write_int(w, arr.len().try_into().unwrap());
                for x in arr.iter().copied() {
                    write_int(w, x);
                }
            }
            Nbt::LongArray(arr) => {
                write_tagtype(w, TagType::LongArray);
                write_ushort_string(w, prop_name);
                write_int(w, arr.len().try_into().unwrap());
                for x in arr.iter().copied() {
                    write_long(w, x);
                }
            }
        }
    }
    write_tagtype(w, TagType::End);
}

pub(crate) fn write_compound_nbt<W: Write>(w: &mut W, nbt: &CompoundNbt<'_>) {
    write_tagtype(w, TagType::Compound);
    write_compound_nbt_no_tagtype(w, nbt);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compound_usage() {
        fn foo<'a, 'b>(x: &'a CompoundNbt<'b>) -> (Option<&'a Nbt<'b>>, Option<&'a Nbt<'b>>) {
            (x.get("hello"), x.get("goodbye"))
        }

        let mut c = CompoundNbt::new("hi");
        c.set("hello", Nbt::Byte(3));
        c.set(String::from("goodbye"), Nbt::Byte(12));

        assert!(matches!(
            foo(&c),
            (Some(&Nbt::Byte(3)), Some(&Nbt::Byte(12)))
        ));
    }

    #[test]
    fn basic_nbt_serde() {
        let buf = [
            0x0a, 0x00, 0x0b, 0x68, 0x65, 0x6c, 0x6c, 0x6f, 0x20, 0x77, 0x6f, 0x72, 0x6c, 0x64,
            0x08, 0x00, 0x04, 0x6d, 0x65, 0x6d, 0x65, 0x00, 0x09, 0x42, 0x61, 0x6e, 0x61, 0x6e,
            0x72, 0x61, 0x6d, 0x61, 0x00,
        ];
        let mut x = buf.as_slice();

        let compound = Nbt::read_compound(&mut x);
        assert_eq!(compound.name(), "hello world");
        let foo = compound.get("meme").unwrap();
        let Nbt::String(s) = foo else {
            panic!("expected string, got {foo:?}");
        };
        assert_eq!(s, "Bananrama");

        let mut deserialized = Vec::with_capacity(buf.len());
        write_compound_nbt(&mut deserialized, &compound);
        assert_eq!(buf.as_slice(), &deserialized);
    }
}
