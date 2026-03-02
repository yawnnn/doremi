use anyhow::Context;
use date::interval::{DateInterval, MonthInterval};
use datetime::{Date, DateTime, FromDate, interval::TimeInterval};
use serde::de::{Deserializer, Error};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{self, Debug},
    fs,
    io::{self, Seek},
    iter, ops, path,
    str::FromStr,
};

#[derive(Debug)]
pub struct DB {
    pub dir: path::PathBuf,
    pub meta: Metadata,
}

impl DB {
    pub fn load<P: AsRef<path::Path>>(dir: &P) -> anyhow::Result<Self> {
        if !fs::exists(dir)? {
            fs::create_dir_all(dir)?;
        }
        let meta = Metadata::load(dir)?;
        Ok(Self {
            dir: dir.as_ref().to_path_buf(),
            meta,
        })
    }

    pub fn block_flname(&self, ym: YearMonth) -> path::PathBuf {
        self.dir.join(format!("{ym}.md"))
    }

    pub fn insert(&mut self, rec: &Record, ctime: DateTime) -> anyhow::Result<u64> {
        let flname = self.block_flname(ctime.date().into());
        let mut fl = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(false)
            .create(true)
            .open(flname)?;
        fl.seek(io::SeekFrom::End(0))?;
        rec.write(&mut fl)?;
        self.meta.insert(rec.id, ctime)?;
        self.meta.dump(&self.dir)?;

        Ok(rec.id)
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
        for (ym, month_meta) in &src.meta.months {
            let mut new_recs = Vec::new();
            let mut mod_recs = Vec::new();

            let mut src_fl = fs::File::open(src.block_flname(*ym))?;
            let mut dst_fl = fs::File::open(dst.block_flname(*ym))?;

            for (id, src_meta) in &month_meta.0 {
                match dst.meta.get(*id) {
                    Some(dst_meta) => {
                        assert!(src_meta.ctime == dst_meta.ctime);
                        // TODO: if i want a more meaningful sync, i need more info
                        let (fl, utime) = match src_meta.utime.cmp(&dst_meta.utime) {
                            std::cmp::Ordering::Equal => continue,
                            std::cmp::Ordering::Greater => (&mut src_fl, src_meta.utime),
                            std::cmp::Ordering::Less => (&mut dst_fl, dst_meta.utime),
                        };
                        let rec = DB::find(fl, *id)?;
                        mod_recs.push((rec, utime));
                    }
                    None => new_recs.push((*id, src_meta.ctime)),
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

// TODO: use RDateTime?
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
            fs::File::create(flname)?;
            Ok(Metadata::default())
        }
    }

    fn dump<P: AsRef<path::Path>>(&self, dir: &P) -> anyhow::Result<()> {
        let mut fl = fs::File::create(Self::flname(dir))?;
        serde_json::to_writer_pretty(&mut fl, self)?;
        Ok(())
    }

    fn insert(&mut self, id: u64, ctime: DateTime) -> anyhow::Result<()> {
        let rec_meta = RecMetadata {
            ctime,
            utime: ctime,
        };
        self.months
            .entry(ctime.date().into())
            .and_modify(|month_meta| {
                month_meta.0.insert(id, rec_meta);
            })
            .or_insert(MonthMetadata(HashMap::from([(id, rec_meta)])));

        Ok(())
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

// TODO: remove this
/// Record's DateTime
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RDateTime(DateTime);

impl RDateTime {
    const FMT: &str = "%Y-%m-%d %H:%M:%S";

    fn with_date_and_time(date: Date, time: DateTime) -> RDateTime {
        RDateTime(date.hms(time.hour(), time.minute(), time.second()).build())
    }
}

impl ops::Deref for RDateTime {
    type Target = DateTime;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<DateTime> for RDateTime {
    fn from(value: DateTime) -> Self {
        RDateTime(DateTime::from_timestamp(value.as_seconds(), 0))
    }
}

impl ops::Add<TimeInterval> for RDateTime {
    type Output = Self;
    fn add(self, rhs: TimeInterval) -> Self::Output {
        RDateTime(self.0 + rhs)
    }
}

impl ops::Add<DateInterval> for RDateTime {
    type Output = Self;
    fn add(self, rhs: DateInterval) -> Self::Output {
        let date = self.0.date() + rhs;
        Self::with_date_and_time(date, self.0)
    }
}

impl ops::Add<MonthInterval> for RDateTime {
    type Output = Self;
    fn add(self, rhs: MonthInterval) -> Self::Output {
        let date = self.0.date() + rhs;
        Self::with_date_and_time(date, self.0)
    }
}

impl FromStr for RDateTime {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(RDateTime(DateTime::parse(s, Self::FMT).map_err(|_| ())?))
    }
}

impl fmt::Display for RDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.0.format(Self::FMT))
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
