use datetime::DateTime;
use std::{
    fmt::{self, Debug, Write},
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
struct RDateTime(DateTime);

impl RDateTime {
    const FMT: &str = "%Y-%m-%d %H:%M:%S";

    fn inner(self) -> DateTime {
        self.0
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
        fmt::Display::fmt(&self.0.format(Self::FMT), f)
    }
}

impl From<DateTime> for RDateTime {
    fn from(value: DateTime) -> Self {
        RDateTime(value)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Record {
    id: u32,
    ctime: RDateTime,
    utime: RDateTime,
    tags: RVec<String>,
    name: String,
    data: String,
}

impl Record {
    pub const SEP: &str = "\n---\n";

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
        write!(f, "{}", self.data.clone())
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
        let data = it.map(str::trim).collect::<Vec<_>>().join("\n"); // ignore only trailing endl

        Ok(Record {
            id,
            ctime,
            utime,
            tags,
            name,
            data,
        })
    }
}

fn select<R: std::io::Read>(mut r: R) -> impl Iterator<Item = Record> {
    let mut buf = String::new();
    r.read_to_string(&mut buf).unwrap();
    let mut pos = 0;

    std::iter::from_fn(move || {
        let s = &buf[pos..];
        let (sr, rest) = s.split_once(Record::SEP).unwrap_or((s, ""));
        let r = sr.trim().parse::<Record>().ok()?;
        pos = buf.len() - rest.len();

        Some(r)
    })
}

fn insert<W: std::fmt::Write + ?Sized>(w: &mut W, records: &[Record]) -> std::fmt::Result {
    for r in records.iter() {
        w.write_str(&r.to_string())?;
        w.write_str(Record::SEP)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use datetime::datetime;

    #[test]
    fn to_string() {
        let mut buf = String::new();

        let ctime = datetime!(2026-01-27 10:22:01).into();
        let utime = datetime!(2026-01-27 10:22:01).into();
        let r = Record {
            id: 0,
            ctime: ctime,
            utime: utime,
            tags: RVec(vec!["tag1".to_string(), "tag2".to_string()]),
            name: "note 1".to_string(),
            data: "multiline\ndata".to_string(),
        };
        insert(&mut buf, &[r]).unwrap();

        let ctime = datetime!(2026-01-27 10:22:02).into();
        let utime = datetime!(2026-01-27 20:22:02).into();
        let r = Record {
            id: 1,
            ctime: ctime,
            utime: utime,
            tags: RVec(vec!["tag1".to_string(), "tag2".to_string()]),
            name: "note 1".to_string(),
            data: "one-line data".to_string(),
        };
        insert(&mut buf, &[r]).unwrap();

        let s = "
id = 0
ctime = 2026-01-27 10:22:01
utime = 2026-01-27 10:22:01
tags = [tag1, tag2]
name = note 1
multiline
data
---
id = 1
ctime = 2026-01-27 10:22:02
utime = 2026-01-27 20:22:02
tags = [tag1, tag2]
name = note 1
one-line data
---
";
        let s = s.trim_start();

        assert_eq!(s, buf.as_str(),)
    }

    #[test]
    fn from_str() {
        let s = "
id = 0
ctime = 2026-01-27 10:22:01
utime = 2026-01-27 10:22:01
tags = [tag1, tag2]
name = note 1
multiline
data
---
id = 1
ctime = 2026-01-27 10:22:02
utime = 2026-01-27 20:22:02
tags = [tag1, tag2]
name = note 1
one-line data
---
";
        let s = s.trim_start();

        let mut it = select(s.as_bytes());

        let ctime = datetime!(2026-01-27 10:22:01).into();
        let utime = datetime!(2026-01-27 10:22:01).into();

        assert_eq!(
            Some(Record {
                id: 0,
                ctime: ctime,
                utime: utime,
                tags: RVec(vec!["tag1".to_string(), "tag2".to_string()]),
                name: "note 1".to_string(),
                data: "multiline\ndata".to_string(),
            }),
            it.next()
        );

        let ctime = datetime!(2026-01-27 10:22:02).into();
        let utime = datetime!(2026-01-27 20:22:02).into();

        assert_eq!(
            Some(Record {
                id: 1,
                ctime: ctime,
                utime: utime,
                tags: RVec(vec!["tag1".to_string(), "tag2".to_string()]),
                name: "note 1".to_string(),
                data: "one-line data".to_string(),
            }),
            it.next()
        );
    }
}
