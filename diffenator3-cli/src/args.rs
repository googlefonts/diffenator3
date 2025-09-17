use std::path::PathBuf;

use clap::{builder::ArgAction, Parser};
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Number of worker processes. Defaults to the number of logical CPUs.
    #[clap(short = 'J', long)]
    pub jobs: Option<usize>,

    /// Don't show diffs in font tables
    #[clap(long = "no-tables", action = ArgAction::SetFalse, help_heading = Some("Tests to run"))]
    pub tables: bool,

    /// Show diffs in font tables [default]
    #[clap(long = "tables", overrides_with = "tables", help_heading = Some("Tests to run"))]
    pub _no_tables: bool,

    /// Don't show diffs in font kerning pairs
    #[clap(long = "no-kerns", action = ArgAction::SetFalse, help_heading = Some("Tests to run"))]
    pub kerns: bool,

    /// Show diffs in font kerning pairs [default]
    #[clap(long = "kerns", overrides_with = "kerns", help_heading = Some("Tests to run"))]
    pub _no_kerns: bool,

    /// Don't show diffs in glyph images
    #[clap(long = "no-glyphs", action = ArgAction::SetFalse, help_heading = Some("Tests to run"))]
    pub glyphs: bool,

    /// Show diffs in glyph images [default]
    #[clap(long = "glyphs", overrides_with = "glyphs", help_heading = Some("Tests to run"))]
    pub _no_glyphs: bool,

    /// Don't show diffs in word images
    #[clap(long = "no-words", action = ArgAction::SetFalse, help_heading = Some("Tests to run"))]
    pub words: bool,

    /// Show diffs in word images [default]
    #[clap(long = "words", overrides_with = "words", help_heading = Some("Tests to run"))]
    pub _no_words: bool,

    /// Don't show language support differences
    #[clap(long = "no-languages", action = ArgAction::SetFalse, help_heading = Some("Tests to run"))]
    pub languages: bool,

    /// Show language support differences [default]
    #[clap(long = "languages", overrides_with = "languages", help_heading = Some("Tests to run"))]
    pub _no_languages: bool,

    /// Custom word list files for testing
    #[clap(long = "custom-wordlists", help_heading = Some("Tests to run"))]
    pub custom_wordlists: Vec<PathBuf>,

    /// Show diffs as JSON
    #[clap(long = "json", help_heading = Some("Report format"))]
    pub json: bool,
    /// Show diffs as HTML
    #[clap(long = "html", help_heading = Some("Report format"))]
    pub html: bool,
    /// If an entry is absent in one font, show the data anyway
    #[clap(long = "no-succinct", action = ArgAction::SetFalse, help_heading = Some("Report format"))]
    pub succinct: bool,

    /// If an entry is absent in one font, just report it as absent
    #[clap(long = "succinct", overrides_with = "succinct", help_heading = Some("Report format"))]
    pub _no_succinct: bool,

    /// Maximum number of changes to report before giving up
    #[clap(long = "max-changes", default_value = "128", help_heading = Some("Report format"))]
    pub max_changes: usize,

    /// Indent JSON
    #[clap(long = "pretty", requires = "json", help_heading = Some("Report format"))]
    pub pretty: bool,

    /// Output directory for HTML
    #[clap(long = "output", default_value = "out", requires = "html", help_heading = Some("Report format"))]
    pub output: String,

    /// Directory for custom templates
    #[clap(long = "templates", requires = "html", help_heading = Some("Report format"))]
    pub templates: Option<String>,

    /// Update diffenator3's stock templates
    #[clap(long = "update-templates", requires = "html", help_heading = Some("Report format"))]
    pub update_templates: bool,

    /// Location in user space, in the form axis=123,other=456 (may be repeated)
    #[clap(long = "location", help_heading = "Locations to test")]
    pub location: Vec<String>,
    /// Instance to compare (may be repeated; use * for all instances)
    #[clap(long = "instance", help_heading = "Locations to test")]
    pub instance: Vec<String>,
    /// Masters (as detected from the gvar table)
    #[clap(long = "masters", help_heading = "Locations to test")]
    pub masters: bool,
    /// Cross-product (use min/default/max of all axes)
    #[clap(long = "cross-product", help_heading = "Locations to test")]
    pub cross_product: bool,
    /// Cross-product splits
    #[clap(
        long = "cross-product-splits",
        help_heading = "Locations to test",
        default_value = "1"
    )]
    pub splits: usize,

    /// Don't try to match glyph names between fonts
    #[clap(long = "no-match", help_heading = Some("Report format"))]
    pub no_match: bool,

    /// Show diffs as JSON
    #[clap(long = "quiet")]
    pub quiet: bool,

    /// The first font file to compare
    pub font1: PathBuf,
    /// The second font file to compare
    pub font2: PathBuf,
}
