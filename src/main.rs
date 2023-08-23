use anyhow::Context;
use oreneo::page::Page;

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() > 2 {
        println!("Usage: oreneo [PAGE_DIR]. PAGE_DIR is page by default")
    }
    let page_dir = args.get(1).map_or("page", |page_dir| page_dir.as_str());

    for file in std::fs::read_dir(page_dir)
        .context("Failed to read page dir")?
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
