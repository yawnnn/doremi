pub mod google;

use crate::google::*;

use date::interval::{DateInterval, MonthInterval};
use datetime::{Date, DateTime, FromDate, interval::TimeInterval};
use rand::{self, Rng};
use serde::de::{Deserializer, Error};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::{
    collections::HashMap,
    fmt::{self, Debug},
    fs,
    io::{self, Seek},
    iter, ops, path,
    str::FromStr,
};

fn local_basedir() -> path::PathBuf {
    path::PathBuf::from("data")
}

fn local_api_dir() -> path::PathBuf {
    local_basedir().join("api")
}

fn local_db_dir() -> path::PathBuf {
    local_basedir().join("db")
}

pub struct DB {
    pub dir: path::PathBuf,
    pub meta: Metadata,
}

impl DB {
    pub fn load<P: AsRef<path::Path>>(dir: &P) -> anyhow::Result<Self> {
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

    pub fn sync(&mut self, _other: &Self) -> anyhow::Result<()> {
        todo!()
    }
}

// TODO: use RDateTime?
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct RecMetadata {
    pub ctime: DateTime,
    pub utime: DateTime,
    // hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Default, Serialize, Deserialize)]
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
        Self {
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

pub fn new<S: AsRef<str>>(name: &str, tags: &[S], contents: &str) -> anyhow::Result<u64> {
    let mut db = DB::load(&local_db_dir())?;
    let rec = Record::new(rand::rng().next_u64(), name, tags, contents);

    db.insert(&rec, DateTime::now())
}

pub fn search(
    tags: Option<Vec<String>>,
    beg_dt: DateTime,
    end_dt: Option<DateTime>,
) -> anyhow::Result<Vec<Record>> {
    let mut v = Vec::new();
    let mut ym = beg_dt.date();

    let db = DB::load(&local_db_dir())?;

    while end_dt.is_none_or(|end_dt| ym <= end_dt.date()) {
        let flname = db.block_flname(ym.into());
        if !fs::exists(&flname)? {
            break;
        }
        let mut fl = fs::OpenOptions::new()
            .read(true)
            .truncate(false)
            .open(&flname)?;

        v.extend(DB::select(&mut fl).filter(|r| {
            let rec_meta = db.meta.get(r.id).unwrap();

            rec_meta.ctime >= beg_dt
                && end_dt.is_none_or(|end_dt| rec_meta.ctime <= end_dt)
                && tags
                    .as_ref()
                    .map(|tags| tags.iter().all(|t| r.tags.0.contains(t)))
                    .unwrap_or(true)
        }));

        ym = ym + MonthInterval::new(1);
    }

    Ok(v)
}

fn download_remote<P: AsRef<path::Path>>(dir: &P) -> anyhow::Result<()> {
    fs::remove_dir_all(dir)?;
    fs::create_dir(dir)?;

    let api = DriveApi::new(&local_api_dir())?;
    let files = api.list()?;

    for f in files {
        let contents = api.download(&f.id)?;
        let mut fl = fs::File::create(dir.as_ref().join(f.name))?;
        fl.write_all(&contents)?;
    }

    Ok(())
}

fn list_files<P: AsRef<path::Path>>(dir: &P) -> anyhow::Result<Vec<path::PathBuf>> {
    let v = fs::read_dir(dir).map(|dir| {
        dir.into_iter()
            .filter_map(|entry| entry.ok().map(|e| e.path()).filter(|p| p.is_file()))
            .collect::<Vec<_>>()
    })?;

    Ok(v)
}

fn upload_remote<P: AsRef<path::Path>>(dir: &P) -> anyhow::Result<()> {
    let loc_files = list_files(dir)?;
    let api = DriveApi::new(dir)?;

    for file in loc_files {
        let contents = fs::read_to_string(&file)?;
        let stem = file.file_name().unwrap().to_str().unwrap();
        api.upload(stem, contents.as_bytes())?;
    }

    Ok(())
}

pub fn push() -> anyhow::Result<()> {
    let loc_dir = local_db_dir();
    let loc_db = DB::load(&loc_dir)?;

    let rem_dir = local_basedir().join("remote");
    download_remote(&rem_dir)?;
    let mut rem_db = DB::load(&rem_dir)?;

    rem_db.sync(&loc_db)?;
    fs::remove_dir_all(rem_dir)?;

    upload_remote(&loc_dir)
}

pub fn pull() -> anyhow::Result<()> {
    let loc_dir = local_db_dir();
    let mut loc_db = DB::load(&loc_dir)?;

    let rem_dir = local_basedir().join("remote");
    download_remote(&rem_dir)?;
    let rem_db = DB::load(&rem_dir)?;

    loc_db.sync(&rem_db)?;
    fs::remove_dir_all(&loc_dir)?;

    fs::rename(rem_dir, loc_dir).map_err(Into::into)
}

pub fn list_remote() -> anyhow::Result<Vec<DriveFile>> {
    let api = DriveApi::new(&local_api_dir())?;
    api.list()
}

pub fn clear_remote() -> anyhow::Result<()> {
    let api = DriveApi::new(&local_api_dir())?;
    let lst = api.list()?;
    for f in lst {
        api.delete(&f.id)?;
    }
    Ok(())
}

// TODO: add tests for index
#[cfg(test)]
mod tests {
    use datetime::interval::TimeInterval;

    use super::*;
    use std::convert::Infallible;

    struct TestRng(u64);

    impl rand::TryRng for TestRng {
        type Error = Infallible;
        fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> Result<(), Self::Error> {
            panic!("unused");
        }

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            panic!("unused");
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            self.0 += 1;
            Ok(self.0)
        }
    }

    fn fmt_patt1() -> String {
        "id = 1
tags = tag1, tag2
name = note 1
multiline
data
---
id = 2
tags = tag1, tag2
name = note 1
one-line data
---
"
        .into()
    }

    fn fmt_patt2() -> String {
        "id = 1
tags = tag3, tag4
name = test_new
lorem ipsum something something
---
id = 2
tags = test
name = test_new
lorem ipsum something something
---
"
        .into()
    }

    #[test]
    fn test_to_string() {
        let mut buf = Vec::new();

        let r = Record {
            id: 1,
            tags: RVec(vec!["tag1".to_string(), "tag2".to_string()]),
            name: "note 1".to_string(),
            contents: "multiline\ndata".to_string(),
        };
        r.write(&mut buf).unwrap();

        let r = Record {
            id: 2,
            tags: RVec(vec!["tag1".to_string(), "tag2".to_string()]),
            name: "note 1".to_string(),
            contents: "one-line data".to_string(),
        };
        r.write(&mut buf).unwrap();

        let s = fmt_patt1();

        assert_eq!(s, str::from_utf8(&buf).unwrap())
    }

    #[test]
    fn test_from_str() {
        let s = fmt_patt1();

        let mut it = DB::select(s.as_bytes());

        assert_eq!(
            Some(Record {
                id: 1,
                tags: RVec(vec!["tag1".to_string(), "tag2".to_string()]),
                name: "note 1".to_string(),
                contents: "multiline\ndata".to_string(),
            }),
            it.next()
        );

        assert_eq!(
            Some(Record {
                id: 2,
                tags: RVec(vec!["tag1".to_string(), "tag2".to_string()]),
                name: "note 1".to_string(),
                contents: "one-line data".to_string(),
            }),
            it.next()
        );
    }

    #[test]
    fn test_new() {
        let dt1: RDateTime = DateTime::now().into();
        let dt2: RDateTime = dt1 + TimeInterval::new(61, 0);
        let mut rng = TestRng(0);

        assert_eq!(dt1.date(), dt2.date());
        let date = dt1.date();

        let name = "test_new".to_owned();
        let tags1 = ["tag3".to_owned(), "tag4".to_owned()].to_owned();
        let tags2 = ["test".to_owned()].to_owned();
        let data = "lorem ipsum something something".to_owned();

        let mut db = DB::load(&local_db_dir()).unwrap();
        let flname = db.block_flname(date.into());

        if fs::exists(&flname).unwrap() {
            fs::remove_file(&flname).unwrap();
        }

        let r1 = Record {
            id: rng.next_u64(),
            name: name.clone(),
            tags: RVec(tags1.iter().map(|t| t.into()).collect()),
            contents: data.clone(),
        };
        let r2 = Record {
            id: rng.next_u64(),
            name: name.clone(),
            tags: RVec(tags2.iter().map(|t| t.into()).collect()),
            contents: data.clone(),
        };
        db.insert(&r1, *dt1).unwrap();
        db.insert(&r2, *dt2).unwrap();

        let s = fmt_patt2();

        assert_eq!(s, fs::read_to_string(&flname).unwrap());

        fs::remove_file(&flname).unwrap();
    }

    #[test]
    fn test_new_two_months() {
        let dt1: RDateTime = DateTime::now().into();
        let dt2: RDateTime = dt1 + MonthInterval::new(1);
        let mut rng = TestRng(0);

        let name = "test_new".to_owned();
        let tags1 = ["tag3".to_owned(), "tag4".to_owned()].to_owned();
        let tags2 = ["test".to_owned()].to_owned();
        let data = "lorem ipsum something something".to_owned();

        let mut db = DB::load(&local_db_dir()).unwrap();
        let flname1 = db.block_flname(dt1.date().into());
        if fs::exists(&flname1).unwrap() {
            fs::remove_file(&flname1).unwrap();
        }
        let flname2 = db.block_flname(dt2.date().into());
        if fs::exists(&flname2).unwrap() {
            fs::remove_file(&flname2).unwrap();
        }

        let r1 = Record {
            id: rng.next_u64(),
            name: name.clone(),
            tags: RVec(tags1.iter().map(|t| t.into()).collect()),
            contents: data.clone(),
        };
        let r2 = Record {
            id: rng.next_u64(),
            name: name.clone(),
            tags: RVec(tags2.iter().map(|t| t.into()).collect()),
            contents: data.clone(),
        };
        db.insert(&r1, *dt1).unwrap();
        db.insert(&r2, *dt2).unwrap();

        let s = fmt_patt2();
        let x = fs::read_to_string(&flname1).unwrap() + &fs::read_to_string(&flname2).unwrap();

        assert_eq!(s, x);

        fs::remove_file(&flname1).unwrap();
        fs::remove_file(&flname2).unwrap();
    }
}
