use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use rusty_spire_core::{
    CombatCatalog, CombatSetupV1, PolicyKind, SolveLimits, compare, initialize, solve,
};

#[derive(Parser)]
#[command(
    name = "rusty-spire",
    version,
    about = "Deterministic isolated STS2 combat simulator"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Solve(SolveArgs),
    Compare(CompareArgs),
    Validate(ValidateArgs),
    CatalogInfo(CatalogArgs),
}

#[derive(Args)]
struct SolveArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    catalog: PathBuf,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, value_enum)]
    policy: Option<PolicyArg>,
    #[arg(long)]
    allow_debug_rng_overrides: bool,
    #[command(flatten)]
    limits: LimitArgs,
}

#[derive(Args)]
struct CompareArgs {
    #[arg(long)]
    baseline: PathBuf,
    #[arg(long)]
    candidate: PathBuf,
    #[arg(long)]
    catalog: PathBuf,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, value_enum)]
    policy: Option<PolicyArg>,
    #[arg(long)]
    allow_debug_rng_overrides: bool,
    #[command(flatten)]
    limits: LimitArgs,
}

#[derive(Args)]
struct ValidateArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    catalog: PathBuf,
    #[arg(long)]
    allow_debug_rng_overrides: bool,
}

#[derive(Args)]
struct CatalogArgs {
    #[arg(long)]
    catalog: PathBuf,
}

#[derive(Args)]
struct LimitArgs {
    #[arg(long, default_value_t = 100_000)]
    max_states: usize,
    #[arg(long, default_value_t = 50)]
    max_turns: u32,
    #[arg(long, default_value_t = 60.0)]
    timeout_seconds: f64,
}

impl From<&LimitArgs> for SolveLimits {
    fn from(value: &LimitArgs) -> Self {
        Self {
            max_states: value.max_states,
            max_turns: value.max_turns,
            timeout_seconds: value.timeout_seconds,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum PolicyArg {
    MinimizeHpLoss,
}

impl From<PolicyArg> for PolicyKind {
    fn from(value: PolicyArg) -> Self {
        match value {
            PolicyArg::MinimizeHpLoss => Self::MinimizeHpLoss,
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Command::Solve(args) => {
            let catalog = load_catalog(&args.catalog)?;
            let mut setup = load_setup(&args.input)?;
            if let Some(policy) = args.policy {
                setup.policy = policy.into();
            }
            let combat = initialize(&catalog, &setup, args.allow_debug_rng_overrides)?;
            write_json(
                args.output.as_deref(),
                &solve(&catalog, &combat, (&args.limits).into())?,
            )
        }
        Command::Compare(args) => {
            let catalog = load_catalog(&args.catalog)?;
            let mut baseline = load_setup(&args.baseline)?;
            let mut candidate = load_setup(&args.candidate)?;
            if let Some(policy) = args.policy {
                baseline.policy = policy.into();
                candidate.policy = policy.into();
            }
            let baseline = initialize(&catalog, &baseline, args.allow_debug_rng_overrides)?;
            let candidate = initialize(&catalog, &candidate, args.allow_debug_rng_overrides)?;
            write_json(
                args.output.as_deref(),
                &compare(&catalog, &baseline, &candidate, (&args.limits).into())?,
            )
        }
        Command::Validate(args) => {
            let catalog = load_catalog(&args.catalog)?;
            let setup = load_setup(&args.input)?;
            let initialized = initialize(&catalog, &setup, args.allow_debug_rng_overrides)?;
            write_json(
                None,
                &serde_json::json!({
                    "ok": true,
                    "catalog_sha256": catalog.sha256,
                    "setup_hash": initialized.setup_hash,
                    "policy": initialized.policy,
                }),
            )
        }
        Command::CatalogInfo(args) => {
            let catalog = load_catalog(&args.catalog)?;
            write_json(
                None,
                &serde_json::json!({
                    "schema_version": catalog.data.schema_version,
                    "sha256": catalog.sha256,
                    "source": catalog.data.source,
                    "rng_profiles": catalog.data.rng_profiles.keys().collect::<Vec<_>>(),
                    "ascensions": catalog.data.ascensions,
                    "counts": {
                        "characters": catalog.data.characters.len(),
                        "cards": catalog.data.cards.len(),
                        "relics": catalog.data.relics.len(),
                        "powers": catalog.data.powers.len(),
                        "monsters": catalog.data.monsters.len(),
                        "encounters": catalog.data.encounters.len(),
                    }
                }),
            )
        }
    }
}

fn load_catalog(path: &Path) -> Result<CombatCatalog, Box<dyn Error>> {
    Ok(CombatCatalog::from_json(&fs::read(path)?)?)
}

fn load_setup(path: &Path) -> Result<CombatSetupV1, Box<dyn Error>> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn write_json<T: serde::Serialize>(path: Option<&Path>, value: &T) -> Result<(), Box<dyn Error>> {
    let text = serde_json::to_string_pretty(value)? + "\n";
    if let Some(path) = path {
        fs::write(path, text)?;
    } else {
        print!("{text}");
    }
    Ok(())
}
