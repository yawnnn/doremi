use clap::{Args, Parser, Subcommand};
use date::Date;
use datetime::DateTime;

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    New(New),
    Search(Search),
    Push(()),
    Pull(()),
    ListRemote(()),
    ClearRemote(()),
    // Open(Open),
}

#[derive(Args, Debug)]
pub struct New {
    pub name: String,
    pub contents: Option<String>, // if omitted, read from stdin
    #[arg(short, long)]
    pub tags: Option<String>,
}

#[derive(Args, Debug)]
pub struct Search {
    #[arg(short = 'd', long = "beg-time")]
    pub beg_datetime: Option<String>,
    #[arg(short = 'D', long = "end-time")]
    pub end_datetime: Option<String>,
    #[arg(short = 't', long)]
    pub tags: Option<String>,
}

use std::io::{self, Read};

fn contents_or_stdin(data: Option<String>) -> anyhow::Result<String> {
    match data {
        Some(d) => Ok(d),
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            if buf.trim().is_empty() {
                anyhow::bail!("no data provided (neither argument nor stdin)");
            }
            Ok(buf)
        }
    }
}

fn parse_tags(tags: Option<&str>) -> Option<Vec<String>> {
    tags.map(|tags| {
        tags.split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect()
    })
}

// TODO: optional month and year. default time to 23:59:59
fn parse_datetime(s: Option<&str>) -> anyhow::Result<Option<DateTime>> {
    if let Some(s) = s {
        let dt = if s.split_once('T').is_some() {
            DateTime::parse(s, "%Y-%m-%dT%H:%M:%S")
                .or_else(|_| DateTime::parse(s, "%Y-%m-%dT%H:%M"))
                .or_else(|_| DateTime::parse(s, "%Y-%m-%dT%H"))?
        } else {
            let date = Date::parse(s, "%Y-%m-%d")?;
            DateTime::ymd(date.year(), date.month(), date.day()).build()
        };

        return Ok(Some(dt));
    }

    Ok(None)
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Command::New(args) => {
            let contents = contents_or_stdin(args.contents)?;
            let tags = parse_tags(args.tags.as_deref()).unwrap_or_default();
            let id = doremi_core::new(&args.name, tags.as_slice(), &contents);
            println!("new: {id:?}");
        }
        Command::Search(args) => {
            let tags = parse_tags(args.tags.as_deref());
            let now = Date::today_utc();
            let beg_dt = parse_datetime(args.beg_datetime.as_deref())?
                .unwrap_or(DateTime::ymd(now.year(), now.month(), 1).build());
            let end_dt = parse_datetime(args.end_datetime.as_deref())?;
            let res = doremi_core::search(tags, beg_dt, end_dt);
            println!("{res:#?}");
        }
        Command::Push(()) => {
            doremi_core::push()?;
        }
        Command::Pull(()) => {
            doremi_core::pull()?;
        }
        Command::ListRemote(()) => {
            let files = doremi_core::list_remote()?;
            println!("{:#?}", files);
        }
        Command::ClearRemote(()) => {
            doremi_core::clear_remote()?;
        }
    }

    Ok(())
}
