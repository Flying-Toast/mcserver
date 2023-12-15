use crate::proto::*;
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Read;

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
        // we have borrow, egg bacon and borrow, bacon egg borrow and borrow, std::borrow::Borrow::borrow...
        self.props.get(name).map(std::borrow::Borrow::borrow)
    }

    pub fn name(&self) -> &str {
        &self.name
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
    fn basic_nbt_deser() {
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
    }
}
