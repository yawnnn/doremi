pub mod google;

use date::interval::{DateInterval, MonthInterval};
use datetime::{Date, DateTime, FromDate, interval::TimeInterval};
use rand::{self, Rng};
use std::{
    fmt::{self, Debug},
    fs,
    io::{self, Seek, SeekFrom, Write},
    iter, ops, path,
    str::FromStr,
};

use crate::google::*;

/// Record's Vec
/// x, y, z
#[derive(Debug, PartialEq, Eq)]
struct RVec<T>(Vec<T>);

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
    id: u64,
    ctime: RDateTime,
    utime: RDateTime,
    tags: RVec<String>,
    name: String,
    contents: String,
}

impl Record {
    const SEP: &str = "\n---\n"; // TODO: somethign weirder, and/or escape it

    const K_ID: &str = "id";
    const K_CTIME: &str = "ctime"; // creation time
    const K_UTIME: &str = "utime"; // update time
    const K_TAGS: &str = "tags";
    const K_NAME: &str = "name";
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} = {}", Self::K_ID, self.id)?;
        writeln!(f, "{} = {}", Self::K_CTIME, self.ctime)?;
        writeln!(f, "{} = {}", Self::K_UTIME, self.utime)?;
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
        let ctime = parse_kv(it.next().ok_or(())?, Self::K_CTIME)?;
        let utime = parse_kv(it.next().ok_or(())?, Self::K_UTIME).unwrap_or(ctime);
        let tags = parse_kv(it.next().ok_or(())?, Self::K_TAGS)?;
        let name = parse_kv(it.next().ok_or(())?, Self::K_NAME)?;
        let contents = it.map(str::trim).collect::<Vec<_>>().join("\n"); // ignore only trailing endl

        Ok(Record {
            id,
            ctime,
            utime,
            tags,
            name,
            contents,
        })
    }
}

fn storage_path() -> path::PathBuf {
    path::PathBuf::from("data")
}

fn api_storage_path() -> path::PathBuf {
    storage_path().join("api")
}

fn db_storage_path() -> path::PathBuf {
    storage_path().join("db")
}

fn mk_record_path(date: Date) -> path::PathBuf {
    db_storage_path().join(format!("{}_{}.md", date.year(), date.month()))
}

fn select<R: io::Read>(mut r: R) -> impl Iterator<Item = Record> {
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

fn insert<W: io::Write>(w: &mut W, records: &[Record]) -> io::Result<()> {
    for r in records.iter() {
        w.write_all(r.to_string().as_bytes())?;
        w.write_all(Record::SEP.as_bytes())?;
    }
    Ok(())
}

fn new_with<R: Rng, S: AsRef<str>>(
    name: &str,
    tags: &[S],
    contents: &str,
    rng: &mut R,
    ctime: DateTime,
) -> anyhow::Result<u64> {
    let id = rng.next_u64();
    let r = Record {
        id,
        ctime: ctime.into(),
        utime: ctime.into(),
        name: name.into(),
        tags: RVec(tags.iter().map(|t| t.as_ref().into()).collect()),
        contents: contents.into(),
    };
    let flname = mk_record_path(ctime.date());
    let mut fl = fs::OpenOptions::new()
        .write(true)
        .truncate(false)
        .create(true)
        .open(flname)?;
    fl.seek(SeekFrom::End(0))?;
    insert(&mut fl, &[r])?;

    Ok(id)
}

pub fn new<S: AsRef<str>>(name: &str, tags: &[S], contents: &str) -> anyhow::Result<u64> {
    let mut rng = rand::rng();
    let now = DateTime::now();
    new_with(name, tags, contents, &mut rng, now)
}

pub fn search(
    tags: Option<Vec<String>>,
    beg_dt: DateTime,
    end_dt: Option<DateTime>,
) -> anyhow::Result<Vec<Record>> {
    let mut v = Vec::new();
    let mut record_date = beg_dt.date();

    loop {
        let flname = mk_record_path(record_date);
        if !fs::exists(&flname)? {
            break;
        }
        let mut fl = fs::OpenOptions::new()
            .read(true)
            .truncate(false)
            .open(&flname)?;

        v.extend(select(&mut fl).filter(|r| {
            r.ctime.0 >= beg_dt
                && end_dt.map(|dt| r.ctime.0 <= dt).unwrap_or(true)
                && tags
                    .as_ref()
                    .map(|tags| tags.iter().all(|t| r.tags.0.contains(t)))
                    .unwrap_or(true)
        }));

        record_date = record_date + MonthInterval::new(1);
    }

    Ok(v)
}

pub fn list_local() -> anyhow::Result<Vec<path::PathBuf>> {
    let v = fs::read_dir(db_storage_path()).map(|dir| {
        dir.into_iter()
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                if path.is_file() && path.extension().is_some_and(|e| e == "md") {
                    Some(path)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    })?;

    Ok(v)
}

pub fn push() -> anyhow::Result<()> {
    let loc_files = list_local()?;
    let api = DriveApi::new(&api_storage_path())?;
    //let rem_files = api.list();

    for file in loc_files {
        let contents = fs::read_to_string(&file)?;
        let stem = file.file_name().unwrap().to_str().unwrap();
        api.upload(stem, contents.as_bytes())?;
    }

    Ok(())
}

pub fn pull() -> anyhow::Result<()> {
    let dir = db_storage_path();

    fs::remove_dir_all(&dir)?;
    fs::create_dir(&dir)?;

    let api = DriveApi::new(&api_storage_path())?;
    let files = api.list()?;

    for f in files {
        let contents = api.download(&f.id)?;
        let mut fl = fs::File::create(dir.join(f.name))?;
        fl.write_all(&contents)?;
    }

    Ok(())
}

pub fn list_remote() -> anyhow::Result<Vec<DriveFile>> {
    let api = DriveApi::new(&api_storage_path())?;
    api.list()
}

pub fn clear_remote() -> anyhow::Result<()> {
    let api = DriveApi::new(&api_storage_path())?;
    let lst = api.list()?;
    for f in lst {
        api.delete(&f.id)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use date::interval::DateInterval;
    use std::convert::Infallible;

    struct TestRng(u64);

    impl rand::TryRng for TestRng {
        type Error = Infallible;
        fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> Result<(), Self::Error> {
            Ok(()) // unused
        }

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Ok(0) // unused
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            self.0 += 1;
            Ok(self.0)
        }
    }

    fn fmt_patt1(sdt1: &str, sdt2: &str, sdt3: &str) -> String {
        format!(
            "id = 1
ctime = {sdt1}
utime = {sdt1}
tags = tag1, tag2
name = note 1
multiline
data
---
id = 2
ctime = {sdt2}
utime = {sdt3}
tags = tag1, tag2
name = note 1
one-line data
---
"
        )
    }

    fn fmt_patt2(sdt1: &str, sdt2: &str) -> String {
        format!(
            "id = 1
ctime = {sdt1}
utime = {sdt1}
tags = tag3, tag4
name = test_new
lorem ipsum something something
---
id = 2
ctime = {sdt2}
utime = {sdt2}
tags = test
name = test_new
lorem ipsum something something
---
"
        )
    }

    #[test]
    fn test_to_string() {
        let dt1: RDateTime = DateTime::now().into();
        let dt2: RDateTime = dt1 + TimeInterval::new(61, 0);
        let dt3: RDateTime = dt1 + MonthInterval::new(1) + DateInterval::new(1);
        let sdt1 = dt1.to_string();
        let sdt2 = dt2.to_string();
        let sdt3 = dt3.to_string();

        let mut buf = Vec::new();

        let r = Record {
            id: 1,
            ctime: dt1,
            utime: dt1,
            tags: RVec(vec!["tag1".to_string(), "tag2".to_string()]),
            name: "note 1".to_string(),
            contents: "multiline\ndata".to_string(),
        };
        insert(&mut buf, &[r]).unwrap();

        let r = Record {
            id: 2,
            ctime: dt2,
            utime: dt3,
            tags: RVec(vec!["tag1".to_string(), "tag2".to_string()]),
            name: "note 1".to_string(),
            contents: "one-line data".to_string(),
        };
        insert(&mut buf, &[r]).unwrap();

        let s = fmt_patt1(&sdt1, &sdt2, &sdt3);

        assert_eq!(s, str::from_utf8(&buf).unwrap())
    }

    #[test]
    fn test_from_str() {
        let dt1: RDateTime = DateTime::now().into();
        let dt2: RDateTime = dt1 + TimeInterval::new(61, 0);
        let dt3: RDateTime = dt1 + MonthInterval::new(1) + DateInterval::new(1);
        let sdt1 = dt1.to_string();
        let sdt2 = dt2.to_string();
        let sdt3 = dt3.to_string();

        let s = fmt_patt1(&sdt1, &sdt2, &sdt3);

        let mut it = select(s.as_bytes());

        assert_eq!(
            Some(Record {
                id: 1,
                ctime: dt1,
                utime: dt1,
                tags: RVec(vec!["tag1".to_string(), "tag2".to_string()]),
                name: "note 1".to_string(),
                contents: "multiline\ndata".to_string(),
            }),
            it.next()
        );

        assert_eq!(
            Some(Record {
                id: 2,
                ctime: dt2,
                utime: dt3,
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
        let sdt1 = dt1.to_string();
        let sdt2 = dt2.to_string();
        //let mut rng = rand::rngs::Xoshiro256PlusPlus::seed_from_u64(0);
        let mut rng = TestRng(0);

        assert_eq!(dt1.date(), dt2.date());
        let date = dt1.date();

        let name = "test_new".to_owned();
        let tags1 = ["tag3".to_owned(), "tag4".to_owned()].to_owned();
        let tags2 = ["test".to_owned()].to_owned();
        let data = "lorem ipsum something something".to_owned();

        let flname = mk_record_path(date);

        if fs::exists(&flname).unwrap() {
            fs::remove_file(&flname).unwrap();
        }

        new_with(&name, tags1.as_slice(), &data, &mut rng, *dt1).unwrap();
        new_with(&name, tags2.as_slice(), &data, &mut rng, *dt2).unwrap();

        let s = fmt_patt2(&sdt1, &sdt2);

        assert_eq!(s, fs::read_to_string(&flname).unwrap());

        fs::remove_file(&flname).unwrap();
    }

    #[test]
    fn test_new_two_months() {
        let dt1: RDateTime = DateTime::now().into();
        let dt2: RDateTime = dt1 + MonthInterval::new(1);
        let sdt1 = dt1.to_string();
        let sdt2 = dt2.to_string();
        let mut rng = TestRng(0);

        let name = "test_new".to_owned();
        let tags1 = ["tag3".to_owned(), "tag4".to_owned()].to_owned();
        let tags2 = ["test".to_owned()].to_owned();
        let data = "lorem ipsum something something".to_owned();

        let flname1 = mk_record_path(dt1.date());
        if fs::exists(&flname1).unwrap() {
            fs::remove_file(&flname1).unwrap();
        }
        let flname2 = mk_record_path(dt2.date());
        if fs::exists(&flname2).unwrap() {
            fs::remove_file(&flname2).unwrap();
        }

        new_with(&name, tags1.as_slice(), &data, &mut rng, *dt1).unwrap();
        new_with(&name, tags2.as_slice(), &data, &mut rng, *dt2).unwrap();

        let s = fmt_patt2(&sdt1, &sdt2);
        let x = fs::read_to_string(&flname1).unwrap() + &fs::read_to_string(&flname2).unwrap();

        assert_eq!(s, x);

        fs::remove_file(&flname1).unwrap();
        fs::remove_file(&flname2).unwrap();
    }
}
