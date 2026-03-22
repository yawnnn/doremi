use jni::JNIEnv;
use jni::objects::{JClass, JObjectArray, JString};
use jni::sys::jstring;

use crate::*;

#[derive(serde::Serialize)]
struct JniRecord {
    id: u64,
    name: String,
    tags: Vec<String>,
    body: String,
    ctime: i64,
}

impl From<(Record, RecMetadata)> for JniRecord {
    fn from((rec, md): (Record, RecMetadata)) -> Self {
        JniRecord {
            id: rec.id,
            name: rec.name.clone(),
            tags: rec.tags.0.clone(),
            body: rec.contents.clone(),
            ctime: md.ctime.as_milliseconds(),
        }
    }
}

fn jni_get_string(env: &mut JNIEnv, s: JString) -> String {
    env.get_string(&s).expect("Couldn't get java string").into()
}

fn jni_get_arr_string(env: &mut JNIEnv, arr: JObjectArray) -> Vec<String> {
    let len = env
        .get_array_length(&arr)
        .expect("Couldn't get array length");
    (0..len)
        .map(|i| {
            let elem = env
                .get_object_array_element(&arr, i)
                .expect("Couldn't get array element");
            jni_get_string(env, JString::from(elem))
        })
        .collect()
}

fn jni_get_id(env: &mut JNIEnv, s: JString) -> u64 {
    jni_get_string(env, s).parse().expect("Couldn't parse id")
}

fn jni_new_string(env: &mut JNIEnv, s: &str) -> jstring {
    env.new_string(s)
        .expect("Couldn't create java string!")
        .into_raw()
}

fn jni_read_all<P: AsRef<Path>>(dir: &P) -> anyhow::Result<serde_json::Value> {
    let mut v = Vec::new();
    let mut select = Select::new(&dir_db(dir))?;

    while let Some((_, month_md, recs)) = select.next() {
        for r in recs {
            let md = month_md.0.get(&r.id).unwrap();
            v.push(JniRecord::from((r, *md)));
        }
    }
    serde_json::to_value(&v).map_err(|e| e.into())
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_android_doremi_CoreLib_readAll(
    mut env: JNIEnv,
    _class: JClass,
    basedir: JString,
) -> jstring {
    let basedir = jni_get_string(&mut env, basedir);

    let notes_json = jni_read_all(&basedir).unwrap_or_default();

    jni_new_string(&mut env, &notes_json.to_string())
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_android_doremi_CoreLib_new(
    mut env: JNIEnv,
    _class: JClass,
    basedir: JString,
    name: JString,
    tags: JObjectArray,
    body: JString,
    dbg_ctime: jni::sys::jlong,  // TODO: remove
) -> jstring {
    let basedir = jni_get_string(&mut env, basedir);
    let name = jni_get_string(&mut env, name);
    let tags_vec = jni_get_arr_string(&mut env, tags);
    let body = jni_get_string(&mut env, body);
    let dbg_ctime = match dbg_ctime {
        1.. => Some(DateTime::from_timestamp_millis(dbg_ctime)),
        _ => None,
    };

    let (rec, rec_md) =
        crate::new(&basedir, &name, &tags_vec, &body, dbg_ctime).expect("Failed to create note");
    let jni_record = JniRecord::from((rec, rec_md));
    let note_json = serde_json::to_string(&jni_record).expect("Failed to serialize note");

    jni_new_string(&mut env, &note_json.to_string())
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_android_doremi_CoreLib_update(
    mut env: JNIEnv,
    _class: JClass,
    basedir: JString,
    id: JString,
    name: JString,
    tags: JObjectArray,
    body: JString,
) -> jstring {
    let basedir = jni_get_string(&mut env, basedir);
    let id = jni_get_id(&mut env, id);
    let name = jni_get_string(&mut env, name);
    let tags_vec = jni_get_arr_string(&mut env, tags);
    let body = jni_get_string(&mut env, body);

    let rec = Record::new(id, &name, &tags_vec, &body);
    let rec_md = crate::update(&basedir, &rec).expect("Failed to update note");
    let jni_record = JniRecord::from((rec, rec_md));
    let note_json = serde_json::to_string(&jni_record).expect("Failed to serialize note");

    jni_new_string(&mut env, &note_json.to_string())
}
