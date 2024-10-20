extern crate winres;
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_resource_file("resources.rc");
    res.add_toolkit_include(true);
    res.compile().unwrap();
}
