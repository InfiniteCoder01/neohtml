use anyhow::Context;
use oreneo::page::Page;

fn parse_dir<RP, OP, PP>(project_root: RP, output_root: OP, path: PP) -> anyhow::Result<()>
where
    RP: AsRef<std::path::Path>,
    OP: AsRef<std::path::Path>,
    PP: AsRef<std::path::Path>,
{
    let project_root = project_root.as_ref();
    let output_root = output_root.as_ref();
    let path = path.as_ref();
    for file in std::fs::read_dir(project_root.join(path))
        .context("Failed to read page dir")?
        .flatten()
    {
        let file_name = file.file_name();
        let file_name = file_name.to_string_lossy();
        let file_name = file_name.as_ref();
        let page_path = path.join(file_name);
        if page_path.extension().and_then(|ext| ext.to_str()) == Some("neo") {
            let page = Page::load(&project_root.join(&page_path))
                .context(format!("Failed to parse page {:?}!", page_path))?;

            let generated_html = page
                .to_html_string(
                    &pathdiff::diff_paths(
                        ".",
                        page_path.parent().context("Page has no parent???")?,
                    )
                    .context("Failed to construct relative path of project root for page!")?,
                )
                .context(format!("Failed to build page {:?}!", page_path))?;

            let html_path = output_root.join(page_path.with_extension("html"));
            std::fs::write(&html_path, generated_html)
                .context(format!("Failed to write page {html_path:?}!"))?;
        } else if project_root.join(&page_path).is_dir() {
            parse_dir(project_root, output_root, page_path)?;
        }
    }

    Ok(())
}

use clap::Parser;

/// Neopolitan parser and HTML generator
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    /// Page directory. "page" by default
    #[arg(default_value = "page")]
    page_dir: String,

    /// Output directory. "html" by default
    #[arg(short, long, default_value = "html")]
    output: String,
}

fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse();
    parse_dir(&args.page_dir, &args.output, ".")?;
    Ok(())
}
