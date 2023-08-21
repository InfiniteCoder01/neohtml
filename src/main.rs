use anyhow::Context;
use neohtml::page::Page;

fn main() -> anyhow::Result<()> {
    for file in std::fs::read_dir("page")
        .context("Failed to read page folder")?
        .flatten()
    {
        let page_path = file.path();
        if page_path.extension().and_then(|ext| ext.to_str()) == Some("neo") {
            let page = Page::load(file.path())
                .context(format!("Failed to parse page {:?}!", page_path))?;
            let html_path = page_path.with_extension("html");
            std::fs::write(
                &html_path,
                page.to_html_string()
                    .context(format!("Failed to build page {:?}!", page_path))?,
            )
            .context(format!("Failed to write page {html_path:?}!"))?;
        }
    }
    Ok(())
}
