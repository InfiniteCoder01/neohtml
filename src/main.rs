use anyhow::Context;
use oreneo::page::Page;

fn parse_dir<RP, PP>(project_root: RP, path: PP) -> anyhow::Result<()>
where
    RP: AsRef<std::path::Path>,
    PP: AsRef<std::path::Path>,
{
    let project_root = project_root.as_ref();
    for file in std::fs::read_dir(path)
        .context("Failed to read page dir")?
        .flatten()
    {
        let page_path = file.path();
        if page_path.extension().and_then(|ext| ext.to_str()) == Some("neo") {
            let page =
                Page::load(&page_path).context(format!("Failed to parse page {:?}!", page_path))?;

            let html_path = page_path.with_extension("html");
            std::fs::write(
                &html_path,
                page.to_html_string(
                    pathdiff::diff_paths(
                        page_path.parent().context(format!(
                            "Failed to get page's parent! Page path: {:?}",
                            page_path
                        ))?,
                        project_root,
                    )
                    .context(format!(
                        "Failed to determine relative path of page {:?}!",
                        page_path
                    ))?
                    .as_path()
                    .to_string_lossy()
                    .as_ref(),
                )
                .context(format!("Failed to build page {:?}!", page_path))?,
            )
            .context(format!("Failed to write page {html_path:?}!"))?;
        } else if page_path.is_dir() {
            parse_dir(project_root, page_path)?;
        }
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() > 2 {
        println!("Usage: oreneo [PAGE_DIR]. PAGE_DIR is page by default")
    }

    let page_dir = args.get(1).map_or("page", |page_dir| page_dir.as_str());
    parse_dir(page_dir, page_dir)?;

    Ok(())
}
