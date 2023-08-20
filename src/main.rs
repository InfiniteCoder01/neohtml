use neohtml::page::Page;

fn main() {
    std::fs::write(
        "README.html",
        Page::load("README.neo")
            .unwrap()
            .to_html_string()
            .unwrap(),
    )
    .unwrap();
}
