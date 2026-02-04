use std::{
    cmp,
    fmt::{self, Debug, Write},
    str::FromStr,
};

/// [x, y, z]
#[derive(Debug, PartialEq, Eq)]
struct RecordList(Vec<String>);

impl fmt::Display for RecordList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]", self.0.join(", "))
    }
}

impl FromStr for RecordList {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .map(|s| {
                s.split(',')
                    .map(|s| s.trim().to_owned())
                    .collect::<Vec<_>>()
            })
            .map(|v| RecordList(v))
            .ok_or(())
    }
}

#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl Date {
    fn new(year: u16, month: u8, day: u8) -> Self {
        Date { year, month, day }
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl FromStr for Date {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split('-');
        Ok(Date {
            year: it.next().ok_or(())?.parse().map_err(|_| ())?,
            month: it.next().ok_or(())?.parse().map_err(|_| ())?,
            day: it.next().ok_or(())?.parse().map_err(|_| ())?,
        })
    }
}

impl Ord for Date {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.year
            .cmp(&other.year)
            .then(self.month.cmp(&other.month))
            .then(self.day.cmp(&other.day))
    }
}

impl PartialOrd for Date {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
struct Time {
    hour: u8,
    minute: u8,
    second: u8,
}

impl Time {
    fn new(hour: u8, minute: u8, second: u8) -> Self {
        Time {
            hour,
            minute,
            second,
        }
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
}

impl FromStr for Time {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split(':').flat_map(|s| s.parse::<u8>().ok());
        Ok(Time {
            hour: it.next().ok_or(())?,
            minute: it.next().ok_or(())?,
            second: it.next().ok_or(())?,
        })
    }
}

impl Ord for Time {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.hour
            .cmp(&other.hour)
            .then(self.minute.cmp(&other.minute))
            .then(self.second.cmp(&other.second))
    }
}

impl PartialOrd for Time {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// 2026-01-27 10:22:01
#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
struct DateTime {
    date: Date,
    time: Time,
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.date, self.time)
    }
}

impl FromStr for DateTime {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.split_once(' ')
            .and_then(|(date, time)| {
                Some(DateTime {
                    date: date.parse().ok()?,
                    time: time.parse().ok()?,
                })
            })
            .ok_or(())
    }
}

impl Ord for DateTime {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.date.cmp(&other.date).then(self.time.cmp(&other.time))
    }
}

impl PartialOrd for DateTime {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Record {
    id: u32,
    ctime: DateTime,
    utime: DateTime,
    tags: RecordList,
    name: String,
    data: String,
}

impl Record {
    const K_ID: &str = "id";
    const K_CTIME: &str = "ctime"; // creation time
    const K_UTIME: &str = "utime"; // update time
    const K_TAGS: &str = "tags";
    const K_NAME: &str = "name";
    pub const SEP: &str = "\n---\n";
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
        let utime = parse_kv(it.next().ok_or(())?, Self::K_UTIME).unwrap_or_default();
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

    #[test]
    fn to_string() {
        let mut buf = String::new();

        let ctime = DateTime {
            date: Date::new(2026, 01, 27),
            time: Time::new(10, 22, 01),
        };
        let utime = ctime;
        let r = Record {
            id: 0,
            ctime: ctime,
            utime: utime,
            tags: RecordList(vec!["tag1".to_string(), "tag2".to_string()]),
            name: "note 1".to_string(),
            data: "multiline\ndata".to_string(),
        };
        insert(&mut buf, &[r]).unwrap();

        let ctime = DateTime {
            time: Time {
                second: ctime.time.second + 1,
                ..ctime.time
            },
            ..ctime
        };
        let utime = DateTime {
            time: Time {
                hour: 20,
                ..ctime.time
            },
            ..ctime
        };
        let r = Record {
            id: 1,
            ctime: ctime,
            utime: utime,
            tags: RecordList(vec!["tag1".to_string(), "tag2".to_string()]),
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

        let ctime = DateTime {
            date: Date::new(2026, 01, 27),
            time: Time::new(10, 22, 01),
        };
        let utime = ctime;

        assert_eq!(
            Some(Record {
                id: 0,
                ctime: ctime,
                utime: utime,
                tags: RecordList(vec!["tag1".to_string(), "tag2".to_string()]),
                name: "note 1".to_string(),
                data: "multiline\ndata".to_string(),
            }),
            it.next()
        );

        let ctime = DateTime {
            time: Time {
                second: ctime.time.second + 1,
                ..ctime.time
            },
            ..ctime
        };
        let utime = DateTime {
            time: Time {
                hour: 20,
                ..ctime.time
            },
            ..ctime
        };

        assert_eq!(
            Some(Record {
                id: 1,
                ctime: ctime,
                utime: utime,
                tags: RecordList(vec!["tag1".to_string(), "tag2".to_string()]),
                name: "note 1".to_string(),
                data: "one-line data".to_string(),
            }),
            it.next()
        );
    }
}
