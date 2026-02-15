pub mod sync;

use date::interval::{DateInterval, MonthInterval};
use datetime::{Date, DateTime, FromDate, interval::TimeInterval};
use std::{
    fmt::{self, Debug, Write},
    fs,
    io::{self, Seek, SeekFrom},
    iter, ops, path,
    str::FromStr,
};

/// [x, y, z]
#[derive(Debug, PartialEq, Eq)]
struct RVec<T>(Vec<T>);

impl<T: fmt::Display> fmt::Display for RVec<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('[')?;
        for (i, elmt) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", elmt)?
        }
        f.write_char(']')
    }
}

impl<T: FromStr> FromStr for RVec<T> {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .and_then(|s| {
                s.split(',')
                    .map(|s| s.trim().parse().ok())
                    .collect::<Option<Vec<_>>>()
            })
            .map(|v| RVec(v))
            .ok_or(())
    }
}

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
    id: u32,
    ctime: RDateTime,
    utime: RDateTime,
    tags: RVec<String>,
    name: String,
    contents: String,
}

impl Record {
    const SEP: &str = "\n---\n";

    const K_ID: &str = "id";
    const K_CTIME: &str = "ctime"; // creation time
    const K_UTIME: &str = "utime"; // update time
    const K_TAGS: &str = "tags";
    const K_NAME: &str = "name";
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        writeln!(&mut s, "{} = {}", Self::K_ID, self.id)?;
        writeln!(&mut s, "{} = {}", Self::K_CTIME, self.ctime)?;
        writeln!(&mut s, "{} = {}", Self::K_UTIME, self.utime)?;
        writeln!(&mut s, "{} = {}", Self::K_TAGS, self.tags)?;
        writeln!(&mut s, "{} = {}", Self::K_NAME, self.name)?;
        s.write_str(&self.contents)?;

        f.write_str(&s)
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

fn mk_record_path(date: Date) -> path::PathBuf {
    storage_path().join(&format!("{}_{}.md", date.year(), date.month()))
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

fn next_id() -> anyhow::Result<u32> {
    let mut date = DateTime::now().date();

    loop {
        let flname = mk_record_path(date);
        if !fs::exists(&flname)? {
            return Ok(0);
        }
        let mut fl = fs::OpenOptions::new()
            .read(true)
            .truncate(false)
            .open(&flname)?;
        match select(&mut fl).last().map(|r| r.id) {
            Some(id) => return Ok(id + 1),
            _ => date = date - MonthInterval::new(1),
        }
    }
}

pub fn new_record(
    name: String,
    tags: Vec<String>,
    contents: String,
    ctime: DateTime,
) -> anyhow::Result<u32> {
    let id = next_id()?;
    let r = Record {
        id,
        ctime: ctime.into(),
        utime: ctime.into(),
        name,
        tags: RVec(tags),
        contents,
    };
    let flname = mk_record_path(ctime.date());
    let mut fl = fs::OpenOptions::new()
        .write(true)
        .truncate(false)
        .create(true)
        .open(flname)?;
    fl.seek(SeekFrom::End(0))?;
    insert(&mut fl, &[r]).unwrap();

    Ok(id)
}

pub fn list_records(
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

#[cfg(test)]
mod tests {
    use super::*;
    use date::interval::DateInterval;

    fn fmt_patt1(sdt1: &str, sdt2: &str, sdt3: &str) -> String {
        format!(
            "id = 0
ctime = {sdt1}
utime = {sdt1}
tags = [tag1, tag2]
name = note 1
multiline
data
---
id = 1
ctime = {sdt2}
utime = {sdt3}
tags = [tag1, tag2]
name = note 1
one-line data
---
"
        )
    }

    fn fmt_patt2(sdt1: &str, sdt2: &str) -> String {
        format!(
            "id = 0
ctime = {sdt1}
utime = {sdt1}
tags = [test]
name = test_new
lorem ipsum something something
---
id = 1
ctime = {sdt2}
utime = {sdt2}
tags = [test]
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
            id: 0,
            ctime: dt1,
            utime: dt1,
            tags: RVec(vec!["tag1".to_string(), "tag2".to_string()]),
            name: "note 1".to_string(),
            contents: "multiline\ndata".to_string(),
        };
        insert(&mut buf, &[r]).unwrap();

        let r = Record {
            id: 1,
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
                id: 0,
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
                id: 1,
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

        assert_eq!(dt1.date(), dt2.date());
        let date = dt1.date();

        let name = "test_new".to_owned();
        let tags = ["test".to_owned()].to_owned();
        let data = "lorem ipsum something something".to_owned();

        let flname = mk_record_path(date);

        if fs::exists(&flname).unwrap() {
            fs::remove_file(&flname).unwrap();
        }

        new_record(name.clone(), tags.to_vec(), data.clone(), *dt1).unwrap();
        new_record(name.clone(), tags.to_vec(), data.clone(), *dt2).unwrap();

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

        let name = "test_new".to_owned();
        let tags = ["test".to_owned()].to_owned();
        let data = "lorem ipsum something something".to_owned();

        let flname1 = mk_record_path(dt1.date());
        if fs::exists(&flname1).unwrap() {
            fs::remove_file(&flname1).unwrap();
        }
        let flname2 = mk_record_path(dt2.date());
        if fs::exists(&flname2).unwrap() {
            fs::remove_file(&flname2).unwrap();
        }

        new_record(name.clone(), tags.to_vec(), data.clone(), *dt1).unwrap();
        new_record(name.clone(), tags.to_vec(), data.clone(), *dt2).unwrap();

        let s = fmt_patt2(&sdt1, &sdt2);
        let x = fs::read_to_string(&flname1).unwrap() + &fs::read_to_string(&flname2).unwrap();

        assert_eq!(s, x);

        fs::remove_file(&flname1).unwrap();
        fs::remove_file(&flname2).unwrap();
    }
}
