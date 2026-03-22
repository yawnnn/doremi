use anyhow::Context;
use datetime::{Date, DateTime};
use serde::de::{Deserializer, Error};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use std::collections::hash_map;
use std::path::Path;
use std::{
    collections::HashMap,
    fmt::{self, Debug},
    fs,
    io::{self, Seek},
    iter, path,
    str::FromStr,
};

#[derive(Debug)]
pub struct DB {
    pub dir: path::PathBuf,
    pub metadata: Metadata,
}

// TODO: locking
impl DB {
    pub fn load<P: AsRef<path::Path>>(dir: &P) -> anyhow::Result<Self> {
        if !fs::exists(dir)? {
            fs::create_dir_all(dir)?;
        }
        let metadata = Metadata::load(dir)?;
        Ok(Self {
            dir: dir.as_ref().to_path_buf(),
            metadata,
        })
    }

    fn block_flname_in(dir: &path::Path, ym: YearMonth) -> path::PathBuf {
        dir.join(format!("{ym}.md"))
    }

    pub fn block_flname(&self, ym: YearMonth) -> path::PathBuf {
        DB::block_flname_in(&self.dir, ym)
    }

    pub fn insert(&mut self, rec: &Record, ctime: DateTime) -> anyhow::Result<RecMetadata> {
        let flname = self.block_flname(ctime.date().into());
        let mut fl = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(false)
            .create(true)
            .open(flname)?;
        fl.seek(io::SeekFrom::End(0))?;
        rec.write(&mut fl)?;
        let rec_md = self.metadata.insert(rec.id, ctime)?;
        self.metadata.dump(&self.dir)?;

        Ok(rec_md)
    }

    pub fn update(&mut self, rec: &Record) -> anyhow::Result<RecMetadata> {
        let rec_md = self
            .metadata
            .get_mut(rec.id)
            .context(format!("record {} not found", rec.id))?;
        let flname = DB::block_flname_in(&self.dir, rec_md.ctime.date().into());
        // the files are small by desing, so just read and rewrite the whole thing
        let mut fl = fs::OpenOptions::new().read(true).write(true).open(flname)?;
        let recs = DB::select(&mut fl).collect::<Vec<_>>();
        fl.set_len(0)?;
        fl.seek(io::SeekFrom::Start(0))?;
        for xrec in recs {
            if xrec.id == rec.id {
                rec.write(&mut fl)?;
            }
            else {
                xrec.write(&mut fl)?;
            }
        }
        rec_md.utime = DateTime::now();
        let rec_md = *rec_md;
        self.metadata.dump(&self.dir)?;

        Ok(rec_md)
    }

    // TODO: don't read whole file
    // TODO: should seek(0) before read, but then i cant take a buffer in tests
    pub fn select<R: io::Read>(mut r: R) -> impl Iterator<Item = Record> {
        let mut buf = String::new();
        r.read_to_string(&mut buf).unwrap();
        let mut pos = 0;

        iter::from_fn(move || {
            let s = &buf[pos..];
            let (sr, rest) = s.split_once(Record::SEP).unwrap_or((s, ""));
            let r = sr.trim().parse::<Record>().ok()?;
            pos = buf.len() - rest.len();

            Some(r)
        })
    }

    fn find<R: io::Read + io::Seek>(mut r: R, id: u64) -> anyhow::Result<Record> {
        r.seek(io::SeekFrom::Start(0))?;
        DB::select(&mut r)
            .find(|r| r.id == id)
            .context(format!("record {id} not found"))
    }

    // TODO: delay reading records so i dont need to dwld everything
    pub fn sync(dst: &mut DB, src: &DB) -> anyhow::Result<()> {
        for (ym, month_md) in &src.metadata.months {
            let mut new_recs = Vec::new();
            let mut mod_recs = Vec::new();

            let mut src_fl = fs::File::open(src.block_flname(*ym))?;
            let mut dst_fl = fs::File::open(dst.block_flname(*ym))?;

            for (id, src_md) in &month_md.0 {
                match dst.metadata.get(*id) {
                    Some(dst_md) => {
                        assert!(src_md.ctime == dst_md.ctime);
                        // TODO: if i want a more meaningful sync, i need more info
                        let (fl, utime) = match src_md.utime.cmp(&dst_md.utime) {
                            std::cmp::Ordering::Equal => continue,
                            std::cmp::Ordering::Greater => (&mut src_fl, src_md.utime),
                            std::cmp::Ordering::Less => (&mut dst_fl, dst_md.utime),
                        };
                        let rec = DB::find(fl, *id)?;
                        mod_recs.push((rec, utime));
                    }
                    None => new_recs.push((*id, src_md.ctime)),
                }
            }

            for (id, ctime) in new_recs {
                let rec = DB::find(&mut src_fl, id)?;
                dst.insert(&rec, ctime)?;
            }

            for (rec, utime) in mod_recs {
                dst.insert(&rec, utime)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecMetadata {
    pub ctime: DateTime,
    pub utime: DateTime,
    // hash: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthMetadata(pub HashMap<u64, RecMetadata>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct YearMonth(pub u16, pub u8);

impl From<Date> for YearMonth {
    fn from(value: Date) -> Self {
        Self(value.year() as u16, value.month())
    }
}

impl From<YearMonth> for Date {
    fn from(value: YearMonth) -> Self {
        Date::new(value.0 as i16, value.1, 1)
    }
}

impl FromStr for YearMonth {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (year_str, month_str) = s
            .split_once('_')
            .ok_or("invalid format, expected yyyy_mm")?;

        let year: u16 = year_str.parse().map_err(|_| "invalid year")?;
        let month: u8 = month_str.parse().map_err(|_| "invalid month")?;

        Ok(YearMonth(year, month))
    }
}

impl fmt::Display for YearMonth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}_{:02}", self.0, self.1)
    }
}

impl Serialize for YearMonth {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for YearMonth {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(D::Error::custom)
    }
}

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    pub months: HashMap<YearMonth, MonthMetadata>,
}

impl Metadata {
    fn flname<P: AsRef<path::Path>>(dir: &P) -> path::PathBuf {
        dir.as_ref().join("metadata.json")
    }

    fn load<P: AsRef<path::Path>>(dir: &P) -> anyhow::Result<Self> {
        let flname = Self::flname(dir);

        if fs::exists(&flname)? {
            let contents = fs::read_to_string(&flname)?;
            serde_json::from_str(&contents).map_err(Into::into)
        } else {
            Ok(Metadata::default())
        }
    }

    fn dump<P: AsRef<path::Path>>(&self, dir: &P) -> anyhow::Result<()> {
        let mut fl = fs::File::create(Self::flname(dir))?;
        serde_json::to_writer_pretty(&mut fl, self)?;
        Ok(())
    }

    fn insert(&mut self, id: u64, ctime: DateTime) -> anyhow::Result<RecMetadata> {
        let rec_md = RecMetadata {
            ctime,
            utime: ctime,
        };
        self.months
            .entry(ctime.date().into())
            .and_modify(|month_md| {
                month_md.0.insert(id, rec_md);
            })
            .or_insert(MonthMetadata(HashMap::from([(id, rec_md)])));

        Ok(rec_md)
    }

    pub fn get(&self, id: u64) -> Option<&RecMetadata> {
        self.months.values().find_map(|m| m.0.get(&id))
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut RecMetadata> {
        self.months.values_mut().find_map(|m| m.0.get_mut(&id))
    }
}

/// Record's Vec
/// x, y, z
#[derive(Debug, PartialEq, Eq)]
pub struct RVec<T>(pub Vec<T>);

impl<T: fmt::Display> fmt::Display for RVec<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, elmt) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            f.write_str(&elmt.to_string())?;
        }
        Ok(())
    }
}

impl<T: FromStr> FromStr for RVec<T> {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.split(',')
            .map(|s| s.trim().parse().ok())
            .collect::<Option<Vec<_>>>()
            .map(|v| RVec(v))
            .ok_or(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Record {
    pub id: u64,
    pub tags: RVec<String>,
    pub name: String,
    pub contents: String,
}

impl Record {
    const SEP: &str = "\n---\n"; // TODO: somethign weirder, and/or escape it

    const K_ID: &str = "id";
    const K_TAGS: &str = "tags";
    const K_NAME: &str = "name";

    pub fn new<S: AsRef<str>>(id: u64, name: &str, tags: &[S], contents: &str) -> Self {
        Record {
            id,
            name: name.into(),
            tags: RVec(tags.iter().map(|t| t.as_ref().into()).collect()),
            contents: contents.into(),
        }
    }

    pub fn write<W: io::Write>(&self, fl: &mut W) -> io::Result<()> {
        fl.write_all(self.to_string().as_bytes())?;
        fl.write_all(Record::SEP.as_bytes())
    }
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} = {}", Self::K_ID, self.id)?;
        writeln!(f, "{} = {}", Self::K_TAGS, self.tags)?;
        writeln!(f, "{} = {}", Self::K_NAME, self.name)?;
        f.write_str(&self.contents)
    }
}

impl FromStr for Record {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fn parse_kv<T: FromStr>(s: &str, key: &str) -> Result<T, ()> {
            s.strip_prefix(key)
                .and_then(|s| s.trim().strip_prefix('='))
                .and_then(|s| s.trim().parse().ok())
                .ok_or(())
        }

        let mut it = s.lines().map(str::trim);

        let id = parse_kv(it.next().ok_or(())?, Self::K_ID)?;
        let tags = parse_kv(it.next().ok_or(())?, Self::K_TAGS)?;
        let name = parse_kv(it.next().ok_or(())?, Self::K_NAME)?;
        let contents = it.map(str::trim).collect::<Vec<_>>().join("\n"); // ignore only trailing endl

        Ok(Record {
            id,
            tags,
            name,
            contents,
        })
    }
}

pub struct Select {
    dir: path::PathBuf,
    months: hash_map::IntoIter<YearMonth, MonthMetadata>,
}

impl Select {
    pub fn new<P: AsRef<Path>>(dir: &P) -> anyhow::Result<Select> {
        let db = DB::load(dir)?;
        Ok(Select {
            dir: dir.as_ref().into(),
            months: db.metadata.months.into_iter(),
        })
    }

    #[allow(clippy::should_implement_trait)]
    // can't impl Iterator: see issue #63063 <https://github.com/rust-lang/rust/issues/63063>
    pub fn next(&mut self) -> Option<(YearMonth, MonthMetadata, impl Iterator<Item = Record>)> {
        let (ym, md) = self.months.next()?;
        let flname = DB::block_flname_in(&self.dir, ym);
        let fl = fs::File::open(&flname).unwrap();

        Some((ym, md, DB::select(fl)))
    }
}