use neohtml::page::Page;

fn main() {
    std::fs::write(
        "demo/NEO_README.html",
        Page::load("demo/NEO_README.neo")
            .unwrap()
            .to_html_string()
            .unwrap(),
    )
    .unwrap();
}
