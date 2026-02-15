use doremi_core::sync;

fn main() {
    env_logger::init();

    let creds = sync::get_google_api_creds().unwrap();
    println!("{:?}", creds.list());
}
