pub mod google;
pub mod storage;

use crate::google::*;
use crate::storage::*;

use date::interval::MonthInterval;
use datetime::DateTime;
use rand::{self, Rng};
use std::{fs, io::Write, path};

fn local_basedir() -> path::PathBuf {
    path::PathBuf::from("data")
}

fn local_api_dir() -> path::PathBuf {
    local_basedir().join("api")
}

fn local_db_dir() -> path::PathBuf {
    local_basedir().join("db")
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

    DB::sync(&mut rem_db, &loc_db)?;
    fs::remove_dir_all(rem_dir)?;

    upload_remote(&loc_dir)
}

pub fn pull() -> anyhow::Result<()> {
    let loc_dir = local_db_dir();
    let mut loc_db = DB::load(&loc_dir)?;

    let rem_dir = local_basedir().join("remote");
    download_remote(&rem_dir)?;
    let rem_db = DB::load(&rem_dir)?;

    DB::sync(&mut loc_db, &rem_db)?;
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

    fn test_sync_prep() -> anyhow::Result<(DB, DB, DB)> {
        let mut rng = TestRng(0);

        let load_db = |dir: &path::Path| {
            if fs::exists(dir)? {
                fs::remove_dir_all(dir)?;
            }
            DB::load(&dir)
        };
        let mut local_db = load_db(&local_db_dir()).unwrap();
        let mut remote_db = load_db(&local_db_dir().join("remote")).unwrap();
        let mut correct_db = load_db(&local_db_dir().join("correct")).unwrap();

        let insert = |correct: &mut DB, db: &mut DB, r: Record| {
            let now = DateTime::now();
            db.insert(&r, now).unwrap();
            correct.insert(&r, now).unwrap();
        };
        let mut insert_loc_rem = |correct: &mut DB, r: Record| {
            let now = DateTime::now();
            local_db.insert(&r, now).unwrap();
            remote_db.insert(&r, now).unwrap();
            correct.insert(&r, now).unwrap();
        };

        let rec = Record::new(rng.next_u64(), "first", &["tag1"], "data1");
        insert_loc_rem(&mut correct_db, rec);
        let rec = Record::new(rng.next_u64(), "second", &["tag2"], "data2");
        insert_loc_rem(&mut correct_db, rec);

        let rec = Record::new(rng.next_u64(), "remote", &["remote"], "remote");
        insert(&mut correct_db, &mut remote_db, rec);

        let rec = Record::new(rng.next_u64(), "local", &["local"], "local");
        insert(&mut correct_db, &mut local_db, rec);

        Ok((correct_db, local_db, remote_db))
    }

    fn eq_db_records(db1: &DB, db2: &DB) -> bool {
        let flist1 = list_files(&db1.dir).unwrap();
        let flist2 = list_files(&db2.dir).unwrap();

        let mut recs1 = flist1
            .into_iter()
            .filter(|p| p.extension().unwrap() == "md")
            .flat_map(|f| DB::select(fs::File::open(&f).unwrap()))
            .collect::<Vec<_>>();
        let mut recs2 = flist2
            .into_iter()
            .filter(|p| p.extension().unwrap() == "md")
            .flat_map(|f| DB::select(fs::File::open(&f).unwrap()))
            .collect::<Vec<_>>();
        recs1.sort_by_key(|r| r.id);
        recs2.sort_by_key(|r| r.id);

        recs1 == recs2
    }

    #[test]
    fn test_pull() {
        let (correct_db, mut local_db, remote_db) = test_sync_prep().unwrap();
        DB::sync(&mut local_db, &remote_db).unwrap();

        assert_eq!(correct_db.meta, local_db.meta);
        assert!(eq_db_records(&correct_db, &local_db));
    }

    #[test]
    fn test_push() {
        let (correct_db, local_db, mut remote_db) = test_sync_prep().unwrap();
        DB::sync(&mut remote_db, &local_db).unwrap();

        assert_eq!(correct_db.meta, remote_db.meta);
        assert!(eq_db_records(&correct_db, &remote_db));
    }
}
