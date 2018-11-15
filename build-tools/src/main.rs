// Don't add additional noise to the build tools.
// #![deny(warnings)]

#[macro_use]
extern crate log;

mod cargo_watch;
mod mdbook_bin;

use self::cargo_watch::*;
use clap::Shell;
use commandspec::*;
use diesel::connection::Connection;
use diesel::sqlite::SqliteConnection;
use failure::Error;
use log::LevelFilter;
use mdbook::MDBook;
use std::env;
use std::path::Path;
use structopt::clap::AppSettings;
use structopt::StructOpt;
use wasm_bindgen_cli_support::Bindgen;

#[cfg(windows)]
const WEBPACK_PATH: &str = ".\\node_modules\\.bin\\webpack.cmd";

#[cfg(not(windows))]
const WEBPACK_PATH: &str = "./node_modules/.bin/webpack";

fn abs_string_path<P: AsRef<Path>>(path: P) -> Result<String, Error> {
    Ok(Path::new(".")
        .canonicalize()?
        .join(path)
        .canonicalize()?
        .to_string_lossy()
        .into_owned())
}

// Thin wrapper around run()
fn main() {
    // Only call commandspec cleanup on commands that don't invoke cargo-watch.
    // This is a crude but probably correct hueristic to predict when we'll do so.
    if !std::env::args()
        .nth(1)
        .map(|x| x.find("watch").is_some())
        .unwrap_or(false)
    {
        commandspec::cleanup_on_ctrlc();
        env_logger::Builder::from_default_env()
            .filter_level(LevelFilter::Info)
            .init();
    }

    match run() {
        Ok(_) => {}
        Err(ref err) => {
            eprintln!("Error: {}", err);
            ::std::process::exit(1);
        }
    }
}

/// edit-text build tools
#[derive(StructOpt)]
#[structopt(
    name = "tools",
    bin_name = "tools",
    about = "Build tools and commands for developing edit-text.",
    author = ""
)]
enum Cli {
    #[structopt(name = "build")]
    Build { args: Vec<String> },

    #[structopt(name = "book-build", about = "Builds the book.")]
    BookBuild,

    #[structopt(
        name = "book-watch",
        about = "Watches and rebuilds the book."
    )]
    BookWatch,

    #[structopt(name = "ci", about = "Executes testing for CI")]
    Ci {
        #[structopt(
            long = "no-headless",
            help = "Do not run in headless mode."
        )]
        no_headless: bool,
    },

    #[structopt(
        name = "client-proxy",
        about = "Run client code in your terminal."
    )]
    ClientProxy { args: Vec<String> },

    #[structopt(
        name = "client-proxy-build",
        about = "Build the client proxy."
    )]
    ClientProxyBuild { args: Vec<String> },

    #[structopt(
        name = "completions",
        about = "Generates completion scripts for your shell.",
        raw(setting = "AppSettings::Hidden")
    )]
    Completions {
        #[structopt(name = "SHELL")]
        shell: Shell,
    },

    #[structopt(
        name = "deploy",
        about = "Deploy to sandbox.edit.io.",
        raw(setting = "AppSettings::Hidden")
    )]
    Deploy {
        #[structopt(long = "skip-download")]
        skip_download: bool,

        #[structopt(long = "build-only")]
        build_only: bool,
    },

    #[structopt(
        name = "frontend-build",
        about = "Bundle the frontend JavaScript code."
    )]
    FrontendBuild { args: Vec<String> },

    #[structopt(
        name = "frontend-watch",
        about = "Watch the frontend JavaScript code, building continuously."
    )]
    FrontendWatch { args: Vec<String> },

    #[structopt(name = "logs", about = "Dump database logs.")]
    Logs { args: Vec<String> },

    #[structopt(name = "server", about = "Run the edit-text server.")]
    ServerRun {
        #[structopt(long = "log", help = "Export a log")]
        log: bool,
        args: Vec<String>,
    },

    #[structopt(name = "server-build", about = "Build the edit-text server.")]
    ServerBuild { args: Vec<String> },

    #[structopt(
        name = "oatie-build",
        about = "Build the operational transform library.",
        raw(setting = "AppSettings::Hidden")
    )]
    OatieBuild { args: Vec<String> },

    #[structopt(
        name = "replay",
        about = "Replay an edit-text log.",
        raw(setting = "AppSettings::Hidden")
    )]
    Replay { args: Vec<String> },

    #[structopt(
        name = "test",
        about = "Build tools and commands for developing edit-text.",
        author = ""
    )]
    Test {
        #[structopt(
            long = "no-unit",
            help = "Disable unit tests (which are run by default)."
        )]
        no_unit: bool,

        #[structopt(long = "integration", help = "Enable integration tests.")]
        integration: bool,

        args: Vec<String>,
    },

    #[structopt(
        name = "wasm-build",
        about = "Compile the WebAssembly bundle."
    )]
    WasmBuild {
        #[structopt(name = "no-vendor")]
        no_vendor: bool,
    },

    #[structopt(name = "wasm-watch", about = "Watch the WebAssembly bundle.")]
    WasmWatch {
        #[structopt(name = "no-vendor")]
        no_vendor: bool,
    },
}

fn expect_geckodriver() {
    if let Err(_) = command!("geckodriver -V").unwrap().output() {
        panic!("Abort: please ensure the program 'geckodriver' is installed globally.");
    }
}

fn expect_yarn() {
    if let Err(_) = command!("yarn --version").unwrap().output() {
        panic!("Abort: please ensure the program 'yarn' is installed globally.");
    }
}

fn run() -> Result<(), Error> {
    // We want to set this to the executable directly, rather than cargo build,
    // because we can't re-build the currently running executable on Windows.
    #[allow(non_snake_case)]
    let SELF_PATH = if cfg!(windows) {
        vec![".\\target\\debug\\build-tools.exe"]
    } else {
        vec!["./target/debug/build-tools"]
    };

    // Pass arguments directly to subcommands: don't capture -h, -v, or verification
    // Do this by adding "--" into the args flag after the subcommand.
    let mut args = ::std::env::args().collect::<Vec<_>>();

    // We interpret the --release flag at the build level.
    let release = args.iter().find(|x| *x == "--release").is_some();
    args = args.into_iter().filter(|x| *x != "--release").collect();

    // Respect the CLICOLOR env variable.
    let force_color = ::std::env::var("CLICOLOR")
        .map(|x| x == "1")
        .unwrap_or(false);
    let force_color_flag = if force_color {
        Some("--color=always")
    } else {
        None
    };

    // Run the subcommand.
    let parsed_args = Cli::from_iter(args.iter());
    match parsed_args {
        Cli::Ci {
            // TODO make this actually disable headless mode
            no_headless: _no_headless,
        }=> {
            let output = command!(
                "
                    git --no-pager diff --name-only HEAD..origin/master
                "
            )?
            .output()?
            .stdout;

            eprintln!("touched files:");
            String::from_utf8_lossy(&output)
                .lines()
                .for_each(|value| eprintln!(" - {}", value));
            eprintln!();

            // If only the docs/ folder has been modified, we only need
            // to test if ./tools book-build is successful to merge.
            let only_docs = String::from_utf8_lossy(&output)
                .lines()
                .all(|item| Path::new(item).starts_with("docs/"));

            if only_docs {
                eprintln!("ci: building only book");
                execute!(
                    r"
                        {self_path} book-build
                    ",
                    self_path = SELF_PATH,
                )?;
            } else {
                // Build all targets.
                eprintln!("ci: building all");
                execute!(
                    r"
                        {self_path} build
                    ",
                    self_path = SELF_PATH,
                )?;
                eprintln!();

                if cfg!(windows) {
                    // Only perform unit tests on Windows.
                    eprintln!("ci: perform test (windows)");
                    execute!(
                        r"
                            {self_path} test
                        ",
                        self_path = SELF_PATH,
                    )?;
                } else {
                    // Perform integration tests on Posix.
                    eprintln!("ci: perform test (posix)");
                    execute!(
                        r"
                            {self_path} test --integration
                        ",
                        self_path = SELF_PATH,
                    )?;
                    eprintln!();

                    // Test cross-compilation.
                    eprintln!("ci: package binary");
                    execute!(
                        r"
                            {self_path} deploy --build-only
                        ",
                        self_path = SELF_PATH,
                    )?;
                    eprintln!();
                }
            }
        }

        Cli::WasmWatch { no_vendor } => {
            execute!(
                "
                    rustup target add wasm32-unknown-unknown
                "
            )?;

            let _ = no_vendor;

            watchexec::run(watchexec_args(
                "echo [Starting build.] && cargo run --bin build-tools --quiet -- wasm-build && echo [Build complete.]",
                &["edit-frontend/**", "build-tools/**"],
            ))?;
        }

        Cli::WasmBuild { no_vendor } => {
            let release_flag = Some("--release");

            // Install wasm target
            execute!(
                "
                    rustup target add wasm32-unknown-unknown
                "
            )?;

            // Compile edit-client to WebAssembly.
            eprintln!("Building...");
            execute!(
                r"
                    cd edit-client
                    cargo build {release_flag} --lib --target wasm32-unknown-unknown
                ",
                release_flag = release_flag,
            )?;

            // Compile the TypeScript bindings.
            if !no_vendor {
                eprintln!("Vendoring...");

                std::fs::create_dir_all("./edit-frontend/src/bindgen")?;

                let mut b = Bindgen::new();
                b.input_path("./target/wasm32-unknown-unknown/release/edit_client.wasm")
                    // .debug(args.flag_debug)
                    .typescript(true);
                b.generate("./edit-frontend/src/bindgen")?;

                eprintln!("Done.");
            }
        }

        Cli::ClientProxy { args } => {
            let release_flag = if release { Some("--release") } else { None };

            // Compile and run edit-client-proxy.
            execute!(
                r"
                    cd edit-client
                    export MERCUTIO_WASM_LOG=0
                    export RUST_BACKTRACE=1
                    cargo run {release_flag} --bin edit-client-proxy -- {args}
                ",
                release_flag = release_flag,
                args = args,
            )?;
        }

        Cli::ClientProxyBuild { args } => {
            let release_flag = if release { Some("--release") } else { None };

            execute!(
                r"
                    cd edit-client
                    export MERCUTIO_WASM_LOG=0
                    export RUST_BACKTRACE=1
                    cargo build {release_flag} --bin edit-client-proxy -- {args}
                ",
                release_flag = release_flag,
                args = args,
            )?;
        }

        Cli::OatieBuild { args } => {
            let release_flag = if release { Some("--release") } else { None };

            execute!(
                r"
                    cd oatie
                    cargo build {release_flag} {args}
                ",
                release_flag = release_flag,
                args = args,
            )?;
        }

        Cli::ServerRun { log, args } => {
            if release {
                eprintln!("Building and running edit-text server (release mode)...");
            } else {
                eprintln!("Building and running edit-text server (debug mode)...");
            }

            let database_url = "edit-server/edit.sqlite3";
            if !Path::new(database_url).exists() {
                eprintln!("Building database on first startup...");

                use migrations_internals as migrations;
                use std::io::stdout;

                let conn = SqliteConnection::establish(database_url)?;
                migrations::setup_database(&conn)?;
                migrations::run_pending_migrations_in_directory(
                    &conn,
                    Path::new("edit-server/migrations"),
                    &mut stdout(),
                )?;
            } else {
                println!("Database path: edit-server/edit.sqlite3");
            }

            // Build dist folder if it doesn't exist.
            if !Path::new("edit-frontend/dist/edit.js").exists() {
                execute!(
                    r"
                        {self_path} wasm-build
                    ",
                    self_path = SELF_PATH,
                )?;
                execute!(
                    r"
                        {self_path} frontend-build
                    ",
                    self_path = SELF_PATH,
                )?;
            }

            eprintln!("Starting server...");

            let release_flag = if release { Some("--release") } else { None };

            execute!(
                r"
                    cd edit-server
                    export MERCUTIO_WASM_LOG={use_log}
                    export RUST_BACKTRACE=1
                    export DATABASE_URL=edit-server/edit.sqlite3
                    cargo run {force_color_flag} {release_flag} \
                        --bin edit-server -- {args}
                ",
                use_log = if log { 1 } else { 0 },
                release_flag = release_flag,
                force_color_flag = force_color_flag,
                args = args,
            )?;

            eprintln!("Server exited.");
        }

        Cli::ServerBuild { args } => {
            let release_flag = if release { Some("--release") } else { None };

            // Build dist folder if it doesn't exist.
            if !Path::new("edit-frontend/dist/edit.js").exists() {
                execute!(
                    r"
                        {self_path} wasm-build
                    ",
                    self_path = SELF_PATH,
                )?;
                execute!(
                    r"
                        {self_path} frontend-build
                    ",
                    self_path = SELF_PATH,
                )?;
            }

            execute!(
                r"
                    cd edit-server
                    cargo build {force_color_flag} {release_flag} \
                        --bin edit-server {args}
                ",
                release_flag = release_flag,
                force_color_flag = force_color_flag,
                args = args,
            )?;
        }

        Cli::Replay { args } => {
            // let release_flag = if release { Some("--release") } else { None };

            execute!(
                r"
                    cd edit-client
                    export RUST_BACKTRACE=1
                    cargo run --release --bin edit-replay -- {args}
                ",
                // release_flag = release_flag,
                args = args,
            )?;
        }

        Cli::Test {
            no_unit,
            integration,
            args,
        } => {
            if integration {
                expect_geckodriver();
            }

            if !no_unit {
                // Unit test
                eprintln!("[running unit tests]");
                execute!(
                    r"
                        cargo test -- --test-threads=1
                    ",
                )?;
            }

            if integration {
                // Unit test
                // eprintln!();
                eprintln!("[building integration tests]");
                execute!(
                    r"
                        cd build-tools
                        cargo test --features integration integration_ --no-run
                    ",
                )?;

                // Spawn the server for the integration test
                eprintln!();
                eprintln!("[launching persistent edit-text server]");
                let _server_guard = Some(
                    command!(
                        r"
                        {self_path} server {args}
                    ",
                        self_path = SELF_PATH,
                        args = args,
                    )?
                    .scoped_spawn()
                    .unwrap(),
                );

                // Sleep for 3s after server boots.
                ::std::thread::sleep(::std::time::Duration::from_millis(3000));

                // Unit test
                eprintln!();
                eprintln!("[running integration tests]");
                execute!(
                    r"
                        cd build-tools
                        cargo test --features integration integration_ -- --test-threads=1
                    ",
                )?;
            }
        }

        Cli::Build { args } => {
            eprintln!("[wasm-build]");
            execute!(
                r"
                    {self_path} wasm-build {args}
                ",
                self_path = SELF_PATH,
                args = args,
            )?;

            eprintln!();
            eprintln!("[frontend-build]");
            execute!(
                r"
                    {self_path} frontend-build {args}
                ",
                self_path = SELF_PATH,
                args = args,
            )?;

            eprintln!();
            eprintln!("[server-build]");
            execute!(
                r"
                    {self_path} server-build {args}
                ",
                self_path = SELF_PATH,
                args = args,
            )?;

            eprintln!();
            eprintln!("[client-proxy-build]");
            execute!(
                r"
                    {self_path} client-proxy-build {args}
                ",
                self_path = SELF_PATH,
                args = args,
            )?;

            eprintln!();
            eprintln!("[book-build]");
            execute!(
                r"
                    {self_path} book-build {args}
                ",
                self_path = SELF_PATH,
                args = args,
            )?;
        }

        Cli::FrontendBuild { args } => {
            expect_yarn();

            // Install Node dependencies
            execute!(
                r"
                    cd edit-frontend
                    yarn install
                ",
            )?;

            // Compile WebAssembly
            execute!(
                r"
                    {self_path} wasm-build
                ",
                self_path = SELF_PATH,
            )?;

            // Compile TypeScript
            execute!(
                r"
                    cd edit-frontend
                    {webpack_path} \
                        ./src/index.js --mode development --output-filename='edit.js' {args}
                ",
                webpack_path = WEBPACK_PATH,
                args = args,
            )?;
        }

        Cli::FrontendWatch { args } => {
            expect_yarn();

            // Install Node dependencies
            execute!(
                r"
                    cd edit-frontend
                    yarn install
                ",
            )?;

            // Watch WebAssembly
            std::thread::spawn(|| -> Result<(), Error> {
                watchexec::run(watchexec_args(
                    "echo [Starting build.] && cargo run --bin build-tools --quiet -- wasm-build && echo [Build complete.]",
                    &["edit-frontend/**", "build-tools/**"],
                ))?;
                Ok(())
            });

            // Watch TypeScript
            execute!(
                r"
                    cd edit-frontend
                    {webpack_path} --watch \
                        ./src/index.js --mode development --output-filename='edit.js' {args}
                ",
                webpack_path = WEBPACK_PATH,
                args = args,
            )?;
        }

        Cli::Deploy {
            skip_download,
            build_only,
        } => {
            let edit_deploy_url =
                env::var("EDIT_DEPLOY_URL").unwrap_or("sandbox.edit.io".to_string());
            let edit_dokku_name = env::var("EDIT_DOKKU_NAME").unwrap_or("edit-text".to_string());

            // WASM client code
            eprintln!();
            eprintln!("Compiling WebAssembly...");
            execute!(
                "
                    {self_path} wasm-build
                ",
                self_path = SELF_PATH,
            )?;

            // Frontend JavaScript
            eprintln!("Building frontend...");
            execute!(
                r"
                    {self_path} frontend-build
                ",
                self_path = SELF_PATH,
            )?;

            // Install Linux toolchain.
            // NOTE: This seems to error out if it's already installed,
            // so we only perform this check on non-Linux targets.
            if !cfg!(target_os = "linux") {
                eprintln!();
                eprintln!("[installing linux target]");
                execute!(
                    "
                        rustup target add x86_64-unknown-linux-gnu
                    "
                )?;
            }

            // TODO replace --skip-download with a smarter heuristic
            if !skip_download {
                eprintln!();
                eprintln!("[downloading linux dependencies]");

                // TODO replace this with discrete execute! commands.
                sh_execute!(
                    r#"
                        cd {dir_self}

                        set -e
                        set -x

                        LINKROOT="$(pwd)/dist/link"

                        rm -rf $LINKROOT
                        mkdir -p $LINKROOT

                        cd $LINKROOT

                        export URL=http://security.debian.org/debian-security/pool/updates/main/o/openssl/libssl-dev_1.1.0f-3+deb9u2_amd64.deb
                        curl -L -O $URL
                        ar p $(basename $URL) data.tar.xz | tar xvJf -

                        export URL=http://security.debian.org/debian-security/pool/updates/main/o/openssl/libssl1.1_1.1.0f-3+deb9u2_amd64.deb
                        curl -L -O $URL
                        ar p $(basename $URL) data.tar.xz | tar xvJf -

                        export URL=http://ftp.us.debian.org/debian/pool/main/g/glibc/libc6_2.24-11+deb9u3_amd64.deb
                        curl -L -O $URL
                        ar p $(basename $URL) data.tar.xz | tar xvJf -
                    "#,
                    dir_self = abs_string_path(".")?,
                );
            }

            eprintln!();
            eprintln!("[cross-compiling server binary]");
            execute!(
                r#"
                    cd edit-server

                    export LIBRARY_PATH="{dir_link}/usr/lib/x86_64-linux-gnu;{dir_link}/lib/x86_64-linux-gnu"
                    export OPENSSL_LIB_DIR="{dir_link}/usr/lib/x86_64-linux-gnu/"
                    export OPENSSL_DIR="{dir_link}/usr/"
                    export TARGET_CC="x86_64-unknown-linux-gnu-gcc"
                    export TARGET_CFLAGS="-I {dir_link}/usr/include/x86_64-linux-gnu -isystem {dir_link}/usr/include"

                    cargo build --release --target=x86_64-unknown-linux-gnu \
                        --bin edit-server --features 'standalone'

                "#,
                // Must expand absolute path for linking
                dir_link = format!("{}/dist/link", abs_string_path(".")?),
            )?;
            eprintln!();
            eprintln!("Copying directories...");
            execute!(
                "
                    cp target/x86_64-unknown-linux-gnu/release/edit-server dist/deploy
                "
            )?;

            // Shell out for uploading the file to dokku.
            if !build_only {
                eprintln!();
                eprintln!("Uploading...");
                sh_execute!(
                    r#"
                        cd dist/deploy

                        # Doing these two commands as one pipe may cause dokku to hang
                        # (from experience) so first, upload the tarball, then load it.
                        tar c . | bzip2 | ssh root@{dokku_url} "bunzip2 > /tmp/edit.tar"
                        ssh root@{dokku_url} 'cat /tmp/edit.tar | dokku tar:in {dokku_name}'
                    "#,
                    dokku_url = edit_deploy_url,
                    dokku_name = edit_dokku_name,
                )?;
            }
        }

        Cli::BookBuild => {
            let docs_dir = Path::new("docs");
            eprintln!("Building {:?}", docs_dir);

            MDBook::load(&docs_dir)
                .expect("Could not load mdbook")
                .build()
                .expect("Could not build mdbook");
        }

        Cli::BookWatch => {
            let docs_dir = Path::new("docs");
            eprintln!("Building {:?}", docs_dir);

            let args =
                mdbook_bin::serve::make_subcommand().get_matches_from(vec!["mdbook", "serve"]);

            mdbook_bin::serve::execute(&args, &docs_dir).expect("Could not serve mdbook");
        }

        Cli::Completions { shell } => {
            let mut app = Cli::clap();
            app.gen_completions_to("tools", shell, &mut ::std::io::stdout());
        }

        Cli::Logs { args } => {
            execute!(
                r"
                    cd edit-server
                    export RUST_BACKTRACE=1
                    export DATABASE_URL={database_url}
                    cargo run --bin edit-server-logs -- {args}
                ",
                database_url =
                    env::var("DATABASE_URL").unwrap_or("edit-server/edit.sqlite3".to_string()),
                // release_flag = release_flag,
                args = args,
            )?;
        }
    }

    Ok(())
}