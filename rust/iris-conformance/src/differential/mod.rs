//! Differential fuzzing: generate cases, run every driver on them, and fail
//! on the first difference in canonical outcome.
//!
//! Case `index` of a run with `seed` depends on those two numbers and the
//! case roots alone, so a failure replays with `--seed S --start I --cases 1`.
//! Each case runs five requests: parse, read, catalog as JSON and as dzn, and
//! dzn. Outcomes compare as the conformance runner compares them: errors by
//! code, location and record path, data by canonical value. Any difference,
//! and any status that is neither success nor a language error, is a
//! failure. The tool then reduces the case and writes it in the conformance
//! case layout, without an expectation, which the conformance process
//! chooses.

mod generate;
mod mutate;
mod reduce;
mod rng;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::canonical::{Outcome, OutcomeKind};
use crate::case::{Case, Operation, OutputFormat};
use crate::driver::DriverCommand;
use mutate::Seed;
use rng::Rng;

#[derive(Debug)]
pub struct Options {
    /// Driver commands, at least two
    pub drivers: Vec<String>,
    pub seed: u64,
    /// Index of the first case
    pub start: u64,
    pub limit: Limit,
    pub jobs: usize,
    /// Limit for one driver request
    pub timeout: Duration,
    /// Directory for the reproducer of a failure
    pub keep: Option<PathBuf>,
    pub version: String,
    /// Conformance case roots that seed the `mutate` strategy
    pub roots: Vec<PathBuf>,
}

/// When a run stops: after a number of cases, or once a duration has passed.
#[derive(Clone, Copy, Debug)]
pub enum Limit {
    Cases(u64),
    Duration(Duration),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Strategy {
    Generate,
    Mutate,
}

impl Strategy {
    fn label(self) -> &'static str {
        match self {
            Self::Generate => "generate",
            Self::Mutate => "mutate",
        }
    }
}

/// A reader spec, its input and its parameters as a JSON object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedCase {
    pub strategy: Strategy,
    pub spec: Vec<u8>,
    pub input: Vec<u8>,
    pub params: Option<String>,
}

/// Generate case `index` of a run with `seed` with the `generate` strategy.
pub fn generate(seed: u64, index: u64) -> GeneratedCase {
    let generated = generate::generate(Rng::for_case(seed, index));
    GeneratedCase {
        strategy: Strategy::Generate,
        spec: generated.spec.into_bytes(),
        input: generated.input,
        params: generated.params,
    }
}

/// Case `index` of a run with `seed`: generated, or a mutated seed when there
/// are seeds and the case's generator chooses one.
fn case_for(seed: u64, index: u64, seeds: &[Seed]) -> GeneratedCase {
    let mut rng = Rng::for_case(seed, index);
    if seeds.is_empty() || rng.chance(50) {
        return generate(seed, index);
    }
    let mutated = mutate::mutate(&mut rng, seeds);
    GeneratedCase {
        strategy: Strategy::Mutate,
        spec: mutated.spec,
        input: mutated.input,
        params: mutated.params,
    }
}

/// One driver request of a case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Request {
    Parse,
    Read,
    Catalog,
    CatalogDzn,
    Dzn,
}

impl Request {
    const ALL: [Self; 5] = [
        Self::Parse,
        Self::Read,
        Self::Catalog,
        Self::CatalogDzn,
        Self::Dzn,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Read => "read",
            Self::Catalog => "catalog",
            Self::CatalogDzn => "catalog-dzn",
            Self::Dzn => "dzn",
        }
    }

    fn operation(self) -> Operation {
        match self {
            Self::Parse => Operation::Parse,
            Self::Read => Operation::Read,
            Self::Catalog | Self::CatalogDzn => Operation::Catalog,
            Self::Dzn => Operation::Dzn,
        }
    }

    fn output(self) -> OutputFormat {
        match self {
            Self::CatalogDzn | Self::Dzn => OutputFormat::Dzn,
            _ => OutputFormat::Json,
        }
    }
}

/// What one driver returned for one request, and its decoded outcome or
/// the reason it is a hard failure.
#[derive(Debug)]
pub struct Reply {
    pub status: String,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub outcome: Result<Outcome, String>,
}

/// The first failing case of a run, with its reduced form.
#[derive(Debug)]
pub struct Disagreement {
    pub index: u64,
    pub request: Request,
    pub original: GeneratedCase,
    pub reduced: GeneratedCase,
    /// The drivers' replies to the reduced case
    pub replies: Vec<Reply>,
    /// Where the reproducer was written, with `--keep`
    pub kept: Option<PathBuf>,
}

/// Counts of what a run compared.
#[derive(Debug, Default)]
pub struct Tally {
    pub cases: u64,
    pub comparisons: u64,
    pub strategies: BTreeMap<&'static str, u64>,
    pub requests: BTreeMap<&'static str, u64>,
    pub outcomes: BTreeMap<&'static str, u64>,
}

impl Tally {
    fn add(&mut self, other: Self) {
        self.cases += other.cases;
        self.comparisons += other.comparisons;
        for (target, source) in [
            (&mut self.strategies, other.strategies),
            (&mut self.requests, other.requests),
            (&mut self.outcomes, other.outcomes),
        ] {
            for (label, count) in source {
                *target.entry(label).or_default() += count;
            }
        }
    }
}

#[derive(Debug)]
pub struct Report {
    pub seed: u64,
    pub tally: Tally,
    pub disagreement: Option<Disagreement>,
    /// The command that replays the failing case
    pub replay: Option<String>,
}

impl Report {
    pub fn exit_code(&self) -> u8 {
        u8::from(self.disagreement.is_some())
    }

    pub fn render(&self) -> String {
        let mut text = String::new();
        if let (Some(disagreement), Some(replay)) = (&self.disagreement, &self.replay) {
            render_disagreement(&mut text, self.seed, disagreement, replay);
        }
        let tally = &self.tally;
        writeln!(
            text,
            "Differential: seed={} cases={} comparisons={} disagreements={}",
            self.seed,
            tally.cases,
            tally.comparisons,
            u8::from(self.disagreement.is_some())
        )
        .expect("writing to String cannot fail");
        for (label, counts) in [
            ("strategies", &tally.strategies),
            ("requests", &tally.requests),
            ("outcomes", &tally.outcomes),
        ] {
            let counts: Vec<String> = counts
                .iter()
                .map(|(name, count)| format!("{name}={count}"))
                .collect();
            writeln!(text, "  {label}: {}", counts.join(" "))
                .expect("writing to String cannot fail");
        }
        text
    }
}

fn render_disagreement(text: &mut String, seed: u64, disagreement: &Disagreement, replay: &str) {
    let mut line = |line: String| {
        text.push_str(&line);
        text.push('\n');
    };
    line(format!(
        "DISAGREEMENT seed={seed} index={} strategy={} request={}",
        disagreement.index,
        disagreement.original.strategy.label(),
        disagreement.request.label()
    ));
    for (number, reply) in disagreement.replies.iter().enumerate() {
        let outcome = match &reply.outcome {
            Ok(outcome) => format!("{outcome:?}"),
            Err(error) => format!("hard failure: {error}"),
        };
        line(format!(
            "  driver {}: {}; {outcome}",
            number + 1,
            reply.status
        ));
    }
    match &disagreement.kept {
        Some(path) => line(format!("  reproducer: {}", path.display())),
        None => {
            let reduced = &disagreement.reduced;
            line(format!(
                "  reduced spec: {:?}",
                String::from_utf8_lossy(&reduced.spec)
            ));
            line(format!(
                "  reduced input: {:?}",
                String::from_utf8_lossy(&reduced.input)
            ));
            line(format!("  params: {:?}", reduced.params));
        }
    }
    line(format!("  replay: {replay}"));
}

/// Calls of the failure predicate that reduction may spend.
const REDUCTION_BUDGET: usize = 500;

pub fn run(options: &Options) -> Result<Report, String> {
    validate(options)?;
    let drivers: Vec<DriverCommand> = options
        .drivers
        .iter()
        .map(|driver| DriverCommand::parse(driver))
        .collect::<Result<_, _>>()?;
    let seeds = mutate::load_seeds(&options.roots)?;
    let session = Session {
        options,
        drivers: &drivers,
        seeds: &seeds,
        next: AtomicU64::new(options.start),
        stop: AtomicBool::new(false),
        started: Instant::now(),
        failure: Mutex::new(None),
    };
    let tallies: Vec<Result<Tally, String>> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..options.jobs)
            .map(|_| scope.spawn(|| session.work()))
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().expect("a worker panicked"))
            .collect()
    });
    let mut tally = Tally::default();
    for worker in tallies {
        tally.add(worker?);
    }
    let failure = session
        .failure
        .into_inner()
        .expect("no worker panicked holding the lock");
    let Some(failure) = failure else {
        return Ok(Report {
            seed: options.seed,
            tally,
            disagreement: None,
            replay: None,
        });
    };
    let disagreement = settle(options, &drivers, failure)?;
    Ok(Report {
        seed: options.seed,
        tally,
        replay: Some(replay_command(options, disagreement.index)),
        disagreement: Some(disagreement),
    })
}

fn validate(options: &Options) -> Result<(), String> {
    if options.version != "0.1" {
        return Err(format!(
            "unsupported language version {:?}; supported: 0.1",
            options.version
        ));
    }
    if options.drivers.len() < 2 {
        return Err("differential needs at least two --driver commands".into());
    }
    if options.jobs == 0 {
        return Err("jobs must be positive".into());
    }
    if options.timeout.is_zero() {
        return Err("timeout must be positive".into());
    }
    if let Some(keep) = &options.keep
        && keep.exists()
        && fs::read_dir(keep)
            .map_err(|error| format!("{}: {error}", keep.display()))?
            .next()
            .is_some()
    {
        return Err(format!(
            "{}: --keep directory must be empty",
            keep.display()
        ));
    }
    Ok(())
}

/// A failure as a worker found it, before reduction.
struct Failure {
    index: u64,
    request: Request,
    case: GeneratedCase,
}

/// The state the workers share.
struct Session<'a> {
    options: &'a Options,
    drivers: &'a [DriverCommand],
    seeds: &'a [Seed],
    next: AtomicU64,
    stop: AtomicBool,
    started: Instant,
    failure: Mutex<Option<Failure>>,
}

impl Session<'_> {
    /// Compare cases until the limit, or until a worker finds a failure.
    fn work(&self) -> Result<Tally, String> {
        let scratch = tempfile::tempdir().map_err(|error| error.to_string())?;
        let mut tally = Tally::default();
        while !self.stop.load(Ordering::Relaxed) {
            let index = self.next.fetch_add(1, Ordering::Relaxed);
            let admitted = match self.options.limit {
                Limit::Cases(cases) => index - self.options.start < cases,
                Limit::Duration(duration) => self.started.elapsed() < duration,
            };
            if !admitted {
                break;
            }
            let case = case_for(self.options.seed, index, self.seeds);
            match compare(self.drivers, self.options, scratch.path(), &case)? {
                Ok(compared) => {
                    tally.add(compared);
                    *tally.strategies.entry(case.strategy.label()).or_default() += 1;
                }
                Err(request) => {
                    self.stop.store(true, Ordering::Relaxed);
                    let mut failure = self.failure.lock().expect("no worker panicked");
                    // Keep the first failing index, whichever worker finds it first.
                    if failure.as_ref().is_none_or(|known| index < known.index) {
                        *failure = Some(Failure {
                            index,
                            request,
                            case,
                        });
                    }
                }
            }
        }
        Ok(tally)
    }
}

/// Run every request of `case` on every driver. Returns the tally of the
/// agreeing case, or the first request on which the drivers fail to agree.
/// Requests after a parse error that every driver reports are skipped.
fn compare(
    drivers: &[DriverCommand],
    options: &Options,
    scratch: &Path,
    case: &GeneratedCase,
) -> Result<Result<Tally, Request>, String> {
    let mut tally = Tally {
        cases: 1,
        ..Tally::default()
    };
    for request in Request::ALL {
        let replies = ask_all(drivers, options, scratch, case, request)?;
        let Some(kind) = agreed_kind(&replies) else {
            return Ok(Err(request));
        };
        tally.comparisons += 1;
        *tally.requests.entry(request.label()).or_default() += 1;
        *tally.outcomes.entry(kind_label(kind)).or_default() += 1;
        if request == Request::Parse && kind == OutcomeKind::Error {
            break;
        }
    }
    Ok(Ok(tally))
}

fn kind_label(kind: OutcomeKind) -> &'static str {
    match kind {
        OutcomeKind::Data => "data",
        OutcomeKind::Parse => "parse",
        OutcomeKind::Dzn => "dzn",
        OutcomeKind::Error => "error",
    }
}

/// The kind of outcome every reply has, when all decode and are equal.
fn agreed_kind(replies: &[Reply]) -> Option<OutcomeKind> {
    let first = replies.first()?.outcome.as_ref().ok()?;
    replies[1..]
        .iter()
        .all(|reply| reply.outcome.as_ref().is_ok_and(|outcome| outcome == first))
        .then(|| first.kind())
}

fn ask_all(
    drivers: &[DriverCommand],
    options: &Options,
    scratch: &Path,
    case: &GeneratedCase,
    request: Request,
) -> Result<Vec<Reply>, String> {
    let request_case = write_request(scratch, case, request).map_err(|error| error.to_string())?;
    Ok(drivers
        .iter()
        .map(|driver| ask(driver, &request_case, options))
        .collect())
}

/// Write the files of `case` to `directory` and describe them as the
/// driver protocol's request. The expectation is a placeholder; the
/// drivers' outcomes are compared with each other.
fn write_request(
    directory: &Path,
    case: &GeneratedCase,
    request: Request,
) -> std::io::Result<Case> {
    let spec = directory.join("spec.bin");
    let input = directory.join("input.bin");
    fs::write(&spec, &case.spec)?;
    fs::write(&input, &case.input)?;
    let parse = request == Request::Parse;
    Ok(Case {
        path: directory.into(),
        display_path: "differential case".into(),
        operation: request.operation(),
        output: request.output(),
        spec,
        input: (!parse).then_some(input),
        params: case.params.clone().filter(|_| !parse),
        expected: Outcome::Parse,
        known_failures: Vec::new(),
    })
}

fn ask(driver: &DriverCommand, case: &Case, options: &Options) -> Reply {
    match driver.invoke(case, &options.version, options.timeout) {
        Ok(output) => {
            let outcome = match output.status.code() {
                Some(0) => Outcome::decode(case.success_kind(), &output.stdout, &options.version),
                Some(1) => Outcome::decode(OutcomeKind::Error, &output.stdout, &options.version),
                Some(code) => Err(format!("exit {code}")),
                None => Err("terminated by a signal".into()),
            };
            Reply {
                status: output.status.to_string(),
                stdout: output.stdout,
                stderr: output.stderr,
                outcome,
            }
        }
        Err(failure) => Reply {
            status: failure.message.clone(),
            stdout: failure.stdout,
            stderr: failure.stderr,
            outcome: Err(failure.message),
        },
    }
}

/// Reduce the failing case, collect the drivers' replies to the reduced
/// case, and write the reproducer when `--keep` asks for it.
fn settle(
    options: &Options,
    drivers: &[DriverCommand],
    failure: Failure,
) -> Result<Disagreement, String> {
    let scratch = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut io_error = None;
    let reduced = reduce::reduce(&failure.case, REDUCTION_BUDGET, |candidate| {
        match ask_all(drivers, options, scratch.path(), candidate, failure.request) {
            Ok(replies) => agreed_kind(&replies).is_none(),
            Err(error) => {
                io_error.get_or_insert(error);
                false
            }
        }
    });
    if let Some(error) = io_error {
        return Err(error);
    }
    let replies = ask_all(drivers, options, scratch.path(), &reduced, failure.request)?;
    let mut disagreement = Disagreement {
        index: failure.index,
        request: failure.request,
        original: failure.case,
        reduced,
        replies,
        kept: None,
    };
    if let Some(keep) = &options.keep {
        let original_replies = ask_all(
            drivers,
            options,
            scratch.path(),
            &disagreement.original,
            failure.request,
        )?;
        let title = format!(
            "Differential reproducer: seed={} index={} request={}",
            options.seed,
            failure.index,
            failure.request.label()
        );
        write_reproducer(
            &keep.join("original"),
            &disagreement.original,
            failure.request,
            &title,
            options,
            &original_replies,
        )
        .and_then(|()| {
            write_reproducer(
                &keep.join("reduced"),
                &disagreement.reduced,
                failure.request,
                &title,
                options,
                &disagreement.replies,
            )
        })
        .map_err(|error| format!("{}: {error}", keep.display()))?;
        disagreement.kept = Some(keep.clone());
    }
    Ok(disagreement)
}

/// Write `case` to `directory` in the conformance case layout, without an
/// expectation, and record each driver's reply in `drivers.txt`.
fn write_reproducer(
    directory: &Path,
    case: &GeneratedCase,
    request: Request,
    title: &str,
    options: &Options,
    replies: &[Reply],
) -> std::io::Result<()> {
    fs::create_dir_all(directory)?;
    let spec_name = if std::str::from_utf8(&case.spec).is_ok() {
        "spec.iris"
    } else {
        "spec.bin"
    };
    fs::write(directory.join(spec_name), &case.spec)?;
    if request != Request::Parse {
        let input_name = if std::str::from_utf8(&case.input).is_ok() {
            "input.txt"
        } else {
            "input.bin"
        };
        fs::write(directory.join(input_name), &case.input)?;
        if let Some(params) = &case.params {
            fs::write(directory.join("params.json"), params)?;
        }
    }
    let mut metadata = format!(
        "title = {}\noperation = \"{}\"\n",
        toml_string(title),
        request.operation().as_str()
    );
    if request.operation() == Operation::Catalog {
        let output = if request.output() == OutputFormat::Dzn {
            "dzn"
        } else {
            "json"
        };
        metadata.push_str(&format!("output = \"{output}\"\n"));
    }
    fs::write(directory.join("case.toml"), metadata)?;
    let mut drivers = String::new();
    for (number, (command, reply)) in options.drivers.iter().zip(replies).enumerate() {
        let outcome = match &reply.outcome {
            Ok(outcome) => format!("{outcome:?}"),
            Err(error) => format!("hard failure: {error}"),
        };
        drivers.push_str(&format!(
            "driver {}: {command}\nstatus: {}\noutcome: {outcome}\nstdout:\n{}\nstderr:\n{}\n\n",
            number + 1,
            reply.status,
            String::from_utf8_lossy(&reply.stdout),
            String::from_utf8_lossy(&reply.stderr)
        ));
    }
    fs::write(directory.join("drivers.txt"), drivers)
}

fn toml_string(text: &str) -> String {
    toml::Value::String(text.into()).to_string()
}

/// The command that reruns case `index` alone, with the same drivers,
/// timeout, version and roots.
fn replay_command(options: &Options, index: u64) -> String {
    let mut words = vec!["iris-conformance".to_string(), "differential".to_string()];
    for driver in &options.drivers {
        words.push("--driver".into());
        words.push(driver.clone());
    }
    words.extend(
        [
            "--seed",
            &options.seed.to_string(),
            "--start",
            &index.to_string(),
            "--cases",
            "1",
            "--timeout",
            &options.timeout.as_secs_f64().to_string(),
            "--version",
            &options.version,
        ]
        .map(String::from),
    );
    words.extend(
        options
            .roots
            .iter()
            .map(|root| root.to_string_lossy().into_owned()),
    );
    shell_words::join(words)
}
