use vouch_js_lib;
use vouch_lib::extension::FromLib;

fn main() {
    let mut extension = vouch_js_lib::JsExtension::new();
    vouch_lib::extension::commands::run(&mut extension).unwrap();
}
